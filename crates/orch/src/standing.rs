//! Where everything stands for the owner: which chains wait for him and which run, what each agent is doing and what
//! waits on him with it. These are the server's rules, computed here once from the records, so every channel shows the
//! same and none derives them again; views.rs keeps them current and pushes them. Pure.

use std::collections::{HashMap, HashSet};

use model::{Approval, Chain, ChainStatus, Finding, FindingBlocker, FindingStatus, InboxItem, Landing, Verdict};
use wire::app::{Board, BoardColumn, ChainStanding, ChainTone, Column, Wait, WaitKind};

use crate::waiting::waits_on_owner;
use capability::chainasks::{chain_asks_of, origin_label};
use chains::admit::{Horizons, queue_of};
use chains::goal::goal_approvals;
use chains::homes::is_standalone_home;
use shipping::verification::{Changes, of_chain, verification_check};
use wire::app::CheckBack;

/// The board's columns, in order, with what the owner calls them.
const COLUMNS: [(Column, &str, &str); 6] = [
    (
        Column::Needs,
        "Needs you",
        "What only you can settle lands here: a sign-in code, a credential, a key to read. The Assistant decides the rest.",
    ),
    (
        Column::Running,
        "Running",
        "A chain stands here while its Chair and team work on it, and while you have it paused.",
    ),
    (
        Column::Waiting,
        "Waiting",
        "A chain stands here while its Chair waits to check back, until its time comes or a message wakes it.",
    ),
    (
        Column::Watching,
        "Watching",
        "A chain stands here once its change is merged, while its Chair watches a signal until the effect is verified.",
    ),
    (
        Column::Queued,
        "Queue",
        "A new chain waits here for room in the limit: key results with nothing running first, then the soonest horizon, \
         then the oldest; one held says why.",
    ),
    (
        Column::Finished,
        "Finished",
        "A chain lands here once its Chair or you finish it, until it is archived.",
    ),
];

/// Lays chains out on the board from waiting, everything that waits for the owner in its order (`waiting::waits_of`),
/// wakes, what each waiting chain waits for, landed, the chains whose change is on its base, and inbox, the open items.
/// Each column's order says what the owner looks at first: the chain whose first wait comes first in waiting, what changed
/// last, what wakes next, the queue's order (over the objectives' horizons), what finished last.
pub(crate) fn board_of(
    chains: &[Chain],
    wakes: &HashMap<String, CheckBack>,
    landed: &HashSet<String>,
    waiting: Vec<Wait>,
    inbox: &[InboxItem],
    horizons: &Horizons,
) -> Board {
    // each chain's first wait's place in the list
    let mut first: HashMap<&str, usize> = HashMap::new();
    for (i, w) in waiting.iter().enumerate() {
        if let Some(id) = w.chain_id.as_deref() {
            first.entry(id).or_insert(i);
        }
    }
    let wake_at = |id: &str| wakes.get(id).map(|w| w.at).unwrap_or_default();
    let mut by: HashMap<Column, Vec<&Chain>> = HashMap::new();
    let mut archived = 0usize;
    for c in chains {
        if c.archived_at.is_some() {
            archived = archived.saturating_add(1);
            continue;
        }
        let col = column_of(
            c,
            landed.contains(&c.id),
            first.contains_key(c.id.as_str()),
            wakes.get(&c.id).is_some_and(|w| w.watch.is_some()),
        );
        by.entry(col).or_default().push(c);
    }
    let newest = |a: &&Chain, b: &&Chain| b.updated_at.cmp(&a.updated_at);
    for (col, list) in &mut by {
        match col {
            Column::Needs => list.sort_by_key(|c| first.get(c.id.as_str()).copied().unwrap_or(usize::MAX)),
            Column::Running | Column::Finished => list.sort_by(newest),
            Column::Waiting | Column::Watching => list.sort_by_key(|c| wake_at(&c.id)),
            Column::Queued => {
                let on: HashSet<&str> = list.iter().map(|c| c.id.as_str()).collect();
                *list = queue_of(chains, horizons)
                    .into_iter()
                    .filter(|c| on.contains(c.id.as_str()))
                    .collect();
            }
            // a mentor's columns: the owner's board lays no chain out in them
            Column::Asked | Column::Helping => {}
        }
    }

    let known: HashSet<&str> = chains.iter().map(|c| c.id.as_str()).collect();
    let mut b = Board {
        archived,
        ..Board::default()
    };
    for (key, label, empty) in COLUMNS {
        let mut bc = BoardColumn {
            key,
            label: label.to_owned(),
            empty: empty.to_owned(),
            chains: by.get(&key).map(|l| ids(l)).unwrap_or_default(),
            items: Vec::new(),
        };
        if key == Column::Needs {
            bc.items = inbox
                .iter()
                .filter(|it| waits_on_owner(it) && !known.contains(it.home.as_str()) && !is_standalone_home(&it.home))
                .map(|it| it.id.clone())
                .collect();
            b.needs_you = bc.chains.len().saturating_add(bc.items.len());
        }
        b.columns.push(bc);
    }
    b.waiting = waiting;
    b
}

/// The column chain c is in; landed says its change is on its base, waits that something of it waits for the owner
/// (`waiting::chain_waits`), watches that its Chair holds a watch.
pub(crate) fn column_of(c: &Chain, landed: bool, waits: bool, watches: bool) -> Column {
    if waits {
        return Column::Needs;
    }
    match c.status() {
        ChainStatus::Running | ChainStatus::Paused => Column::Running,
        ChainStatus::Waiting if watches && landed => Column::Watching,
        ChainStatus::Waiting => Column::Waiting,
        ChainStatus::Queued => Column::Queued,
        _ => Column::Finished,
    }
}

fn ids(chains: &[&Chain]) -> Vec<String> {
    chains.iter().map(|c| c.id.clone()).collect()
}

/// How a finished chain ended, in the owner's words (ADR-0067, ADR-0087): Done, Waiting to ship, or Not done. `text` is
/// its badge, `why` what is missing or what waits, in full.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Ending {
    /// Waiting to ship: what is left is the landing of the chain's change or putting it live, by who puts it live.
    pub shipping: bool,
    /// Done: the goal judge confirmed every key result.
    pub done: bool,
    pub text: String,
    pub why: String,
}

impl Ending {
    /// The state alone, as an agent's badge says it: Done, Waiting to ship or Not done.
    pub(crate) fn label(&self) -> &'static str {
        if self.done {
            "Done"
        } else if self.shipping {
            "Waiting to ship"
        } else {
            "Not done"
        }
    }
}

const NOT_DONE: &str = "Not done · ";

fn not_done(text: &str) -> Ending {
    Ending {
        text: format!("{NOT_DONE}{text}"),
        ..Ending::default()
    }
}

/// How finished chain c ended, from its goal judge's last verdict among verdicts, why its last finish had none
/// (`c.unjudged`), owners, what is the owner's to settle of it (`owners_to_settle`, as the goal gate reads it), and
/// landings its landings. It is the one place that names it: a chain stored before the rule reads the same way, from the same records.
pub(crate) fn outcome_of(c: &Chain, verdicts: &[Approval], landings: &[Landing], owners: &[FindingBlocker]) -> Ending {
    use model::Unjudged;
    let goal = goal_approvals(of_chain(c, verdicts));
    match c.unjudged {
        Some(Unjudged::Failed) => return not_done("the goal check could not run"),
        Some(Unjudged::Owner) => return not_done("ended by you before it was checked"),
        Some(Unjudged::Stalled) => return not_done("ended with no progress"),
        Some(Unjudged::NoCheck) => return not_done("no goal check judged it"),
        Some(Unjudged::Limit) if goal.is_empty() => return not_done("the goal check failed as often as it may"),
        _ => {}
    }
    let Some(last) = goal.last() else {
        return not_done("the goal was never checked");
    };
    if last.confirms() {
        return Ending {
            done: true,
            text: "Done".to_owned(),
            ..Ending::default()
        };
    }
    let unproven: Vec<&Finding> = last
        .findings
        .iter()
        .filter(|f| f.status != FindingStatus::Met)
        .collect();
    let items = unproven
        .iter()
        .map(|f| {
            if f.item.is_empty() {
                f.blocker.map_or("", |b| b.as_str())
            } else {
                f.item.as_str()
            }
        })
        .collect::<Vec<_>>()
        .join("; ");
    if last.verdict != Verdict::Approve {
        return Ending {
            text: format!("{NOT_DONE}{} of the goal's results still missing", unproven.len()),
            why: items,
            ..Ending::default()
        };
    }
    // An approve leaves unverified only the landing and the release, which no chain's judge can see before they happen
    // (`goal_verdict_error`). Once they are (the change marked live), or for one stored before that rule, it proved
    // nothing more than it says.
    if !owners.is_empty() && unproven.iter().all(|f| f.blocker.is_some_and(|b| owners.contains(&b))) {
        return Ending {
            shipping: true,
            text: format!("Waiting for its release · {}", shipping_left(landings)),
            why: items,
            done: false,
        };
    }
    Ending {
        text: format!("{NOT_DONE}not proven: {items}"),
        why: items,
        ..Ending::default()
    }
}

/// What is left to ship of a chain's work, from its landings: land those on their way, or release what landed (its
/// repository's procedure, ADR-0116); empty when it lands none.
fn shipping_left(landings: &[Landing]) -> String {
    let named = |keep: fn(&Landing) -> bool| {
        landings
            .iter()
            .filter(|l| l.reverts.is_none() && keep(l))
            .map(|l| format!("{} v{}", l.repo, l.version.number))
            .collect::<Vec<_>>()
    };
    let (open, landed) = (named(|l| l.state.open()), named(Landing::on_base));
    if !open.is_empty() {
        format!("land {}", open.join(", "))
    } else if !landed.is_empty() {
        format!("release {}", landed.join(", "))
    } else {
        String::new()
    }
}

/// Why a queued chain's start is held: by the owner's review (`held_for_review`) or by its key result, a share taken or
/// a stall (`held_by_key_result`), as the board's line says, and the words that say why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Held {
    pub line: &'static str,
    pub why: String,
}

/// What `standing_of` reads of a chain besides the chain: where it stands (`col`), its place in the queue (0 when it is
/// not queued), `held` saying why its start is held (none when it is not),
/// `wake` what it waits for (none when nothing), `waits` what of it waits for the owner (`waiting::chain_waits`, in the
/// board's order), `verdicts` its approvals (their notes and the words of their findings are not needed), `owners` what
/// is the owner's to settle of it, `mentors` the mentors helping it (`helping_now`), whose messages wake it while it
/// waits, and `unseen` that it finished after the owner last viewed it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct StandingInput<'a> {
    pub col: Column,
    pub place: usize,
    pub held: Option<&'a Held>,
    pub wake: Option<&'a CheckBack>,
    pub waits: &'a [Wait],
    pub verdicts: &'a [Approval],
    /// Its landings, oldest first.
    pub landings: &'a [Landing],
    pub owners: &'a [FindingBlocker],
    pub mentors: &'a [String],
    /// Its changes as they are now (views.rs `changes_of`), which its verdicts are judged against; none: not read.
    pub current: Option<&'a Changes>,
    pub unseen: bool,
    /// The id of its project (`chains::project::project_of`); none when it has none.
    pub project: Option<&'a str>,
}

/// Where chain c stands, from what `input` says of it.
pub(crate) fn standing_of(c: &Chain, input: StandingInput<'_>) -> ChainStanding {
    let StandingInput {
        col,
        place,
        held,
        wake,
        waits,
        verdicts,
        landings,
        owners,
        mentors,
        current,
        unseen,
        project,
    } = input;
    let status = c.status();
    let finished = if *status == ChainStatus::Done {
        outcome_of(c, verdicts, landings, owners)
    } else {
        Ending::default()
    };
    let mut why = String::new();
    let mut st = ChainStanding {
        chain_id: c.id.clone(),
        place: (place > 0).then_some(place),
        check_back_at: wake.map(|w| w.at),
        asks: chain_asks_of(c),
        from: origin_label(&c.origin).to_owned(),
        verification: verification_check(c, verdicts, landings, current),
        waits: waits.len(),
        unseen,
        project: project.map(str::to_owned),
        ..ChainStanding::default()
    };
    match col {
        Column::Needs => {
            let first = waits.first();
            st.text = "Needs you".to_owned();
            st.tone = if first.is_some_and(|w| w.kind == WaitKind::Failed) {
                ChainTone::Failed
            } else {
                ChainTone::Needs
            };
            // what it asks him to type
            why = first.map_or_else(String::new, |w| w.ask.clone());
            st.wait = first.cloned();
        }
        _ if *status == ChainStatus::Paused => {
            st.text = "Paused".to_owned();
            st.tone = ChainTone::Paused;
        }
        Column::Watching => {
            st.text = "Watching".to_owned();
            st.tone = ChainTone::Watching;
            if let Some(watch) = wake.and_then(|w| w.watch.as_ref()) {
                why.clone_from(&watch.why);
            }
        }
        // What it waits on: the mentors named, a check-back only as wake_at, set when it holds one.
        Column::Waiting => {
            st.text = "Waiting".to_owned();
            st.tone = ChainTone::Waiting;
            if !mentors.is_empty() {
                st.text = format!("Waiting · on {}", mentors.join(", "));
            }
            if let Some(w) = wake {
                why.clone_from(&w.why);
            }
        }
        // a mentor's columns: no chain stands in them for the owner
        Column::Asked | Column::Helping => {}
        Column::Queued => {
            st.tone = ChainTone::Queued;
            match held {
                None => st.text = "Waiting for its turn".to_owned(),
                Some(h) => (st.text, why) = (h.line.to_owned(), h.why.clone()),
            }
        }
        Column::Finished => {
            st.tone = if finished.done {
                ChainTone::Done
            } else if finished.shipping {
                ChainTone::Waiting
            } else {
                ChainTone::NotDone
            };
            st.text = finished.text;
            why = finished.why;
        }
        Column::Running => {
            st.text = "Running".to_owned();
            st.tone = ChainTone::Running;
            st.working = true;
        }
    }
    st.why = Some(why).filter(|w| !w.is_empty());
    st
}
