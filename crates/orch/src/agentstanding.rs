//! Where each agent stands for the owner, as its badge and its line say it, and where it sits in the chain's tree: the
//! server's rules, computed once from the records (standing.rs has the chains'). Pure.

use std::collections::HashMap;

use model::{
    ActorKind, Approval, Card, CardRole, CardStatus, Chain, ChainStatus, Edge, Event, EventStatus, FindingBlocker,
    InboxItem, InboxKind, JudgeKind, Origin, PermissionWait, Time,
};
use wire::app::{AgentStanding, AgentTone, Ask, Branch, DoingTone, OnMessage};

use crate::standing::{Ending, outcome_of};
use crate::waiting::{kind_of, label_of, waits_on_owner};
use chains::cap_names::SOURCE_TOOL_PREFIX;
use chains::outbox::PRIORITY_OPENING;
use chains::schedule::at_work;
use chains::spawn::spawned;
use crew::memory::remembers;
use shipping::land::job_title;

/// What the owner can ask of agent k in a chain whose status is chain, as the server takes it (Prompt, Cancel,
/// StopCard, Restart). The Chair is stopped with its chain, not on its own; a judge that is over can be asked about its
/// answer (its session resumes) and is never restarted (errJudgeRestart). A child card runs in its parent's session
/// (spawn.rs): it reads no message and has no turn of its own, so it is only stopped while it works, and never resumed
/// once it ended.
pub(crate) fn asks_of(k: &Card, chain: &ChainStatus) -> Vec<Ask> {
    let done = *chain == ChainStatus::Done;
    if spawned(k) {
        return if at_work(k) && !done {
            vec![Ask::Stop]
        } else {
            Vec::new()
        };
    }
    if k.status() != &CardStatus::Stopped && k.status() != &CardStatus::Failed && !done {
        let mut asks = vec![Ask::Message];
        if in_turn(k) {
            asks.extend([Ask::Steer, Ask::Interrupt]);
        }
        if k.origin != Origin::Chair {
            asks.push(Ask::Stop);
        }
        return asks;
    }
    if !k.session_id.is_empty() {
        return vec![Ask::Resume];
    }
    if !done && !matches!(CardRole::of(k), CardRole::Judge(_)) {
        return vec![Ask::Restart];
    }
    Vec::new()
}

/// Who works under whom in a chain: the agents hanging under each card, the later agents each works with besides, and
/// the cards that stand beside the Chair, by card id, each oldest first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Tree {
    pub children: HashMap<String, Vec<String>>,
    pub with: HashMap<String, Vec<String>>,
    pub peers: Vec<String>,
}

/// Who works under whom, from chain c's cards and edges: the Chair is the root, and each agent of its team hangs under
/// the earliest agent of the team born before it that it works with (an edge: the work it builds on or judges), else
/// under the Chair; the later ones it works with are its with, so the tree loses no edge of the team. A child card hangs
/// under the card whose process spawned it, and spawns nest. The cards Remit started (the judges) are not the Chair's
/// agents: they stand beside it, its peers, and nothing hangs under them. Children, with and peers are oldest first.
pub(crate) fn tree_of(c: &Chain, cards: &[Card], edges: &[Edge]) -> Tree {
    let team: HashMap<&str, &Card> = cards
        .iter()
        .filter(|k| k.on_team())
        .map(|k| (k.id.as_str(), k))
        .collect();
    let mut born: Vec<&Card> = cards.iter().collect();
    born.sort_by_key(|k| k.created_at);
    let mut t = Tree::default();
    for k in born {
        if k.id == c.chair_id {
            continue;
        }
        if let Some(s) = k.spawn.as_ref().filter(|s| cards.iter().any(|p| p.id == s.parent)) {
            t.children.entry(s.parent.clone()).or_default().push(k.id.clone());
            continue;
        }
        if !k.on_team() {
            t.peers.push(k.id.clone());
            continue;
        }
        // The earlier agents of the team it works with.
        let mut partners: Vec<&Card> = Vec::new();
        for e in edges {
            let other = if e.from == k.id {
                &e.to
            } else if e.to == k.id {
                &e.from
            } else {
                continue;
            };
            if let Some(&p) = team.get(other.as_str())
                && p.created_at < k.created_at
                && !partners.iter().any(|q| q.id == p.id)
            {
                partners.push(p);
            }
        }
        partners.sort_by_key(|p| p.created_at);
        let parent = match partners.split_first() {
            None => c.chair_id.clone(),
            Some((first, rest)) => {
                if !rest.is_empty() {
                    t.with
                        .entry(k.id.clone())
                        .or_default()
                        .extend(rest.iter().map(|p| p.id.clone()));
                }
                first.id.clone()
            }
        };
        t.children.entry(parent).or_default().push(k.id.clone());
    }
    t
}

/// What `agents_of` reads of a chain besides the chain: `cards`, its agents; `edges`, which draw its tree; `verdicts`,
/// its approvals; `owners`, what is the owner's to settle of it; `items`, its open inbox items; `last`, the newest thing
/// each running agent did (store.LastDone); `on_message`, the message each agent last took to open a turn
/// (store.LastTaken), `watches`, what each runs in the background (the titles of its tasks still running); `doing`, what an agent calling one of this server's
/// tools is doing (`tool_doing_of`); `verdict_label`, how a judge's verdict reads (as verification.rs checks a verdict over no current change); `helping`, the mentors whose helps on the
/// chain are open; `checks_back`, that the chain holds a wake (a check-back or a watch); and `landings`, the chain's.
#[derive(Clone, Copy)]
pub(crate) struct AgentsInput<'a> {
    pub cards: &'a [Card],
    pub edges: &'a [Edge],
    pub verdicts: &'a [Approval],
    pub owners: &'a [FindingBlocker],
    pub items: &'a [InboxItem],
    pub last: &'a HashMap<String, Event>,
    pub on_message: &'a HashMap<String, OnMessage>,
    pub watches: &'a HashMap<String, Vec<String>>,
    pub doing: &'a dyn Fn(&str) -> String,
    pub verdict_label: &'a dyn Fn(&Approval) -> String,
    pub helping: &'a [String],
    pub checks_back: bool,
    /// The chain's landings, oldest first.
    pub landings: &'a [model::Landing],
}

/// What one agent's badge and line read besides the agent: what waits on the owner with it, the newest thing it did,
/// the repositories whose submit the server runs for it (jobs), what it runs in the background (watches), the last
/// verdict it gave, if any, and how the chain ended, which its Chair says (none while it has not).
#[derive(Clone, Copy)]
struct CardFacts<'a> {
    waiting: &'a [&'a InboxItem],
    last: Option<&'a Event>,
    jobs: &'a [String],
    watches: &'a [String],
    verdict: Option<&'a Approval>,
    finished: Option<&'a Ending>,
}

/// Where each card of chain c stands, and where it is in the tree its edges draw.
pub(crate) fn agents_of(c: &Chain, input: AgentsInput<'_>) -> Vec<AgentStanding> {
    let cards = input.cards;
    let mut waiting: HashMap<&str, Vec<&InboxItem>> = HashMap::new();
    for it in input.items {
        let who = if cards.iter().any(|k| k.id == it.card_id) {
            it.card_id.as_str()
        } else {
            c.chair_id.as_str()
        };
        waiting.entry(who).or_default().push(it);
    }
    // The repositories the Chair has submitted and whose submit still runs, between its turns.
    let submits: Vec<String> = c
        .repos
        .iter()
        .filter(|rw| rw.submit.is_some())
        .map(|rw| rw.repo.clone())
        .collect();
    // How the chain ended, which its Chair's card says (outcome_of); none while it has not.
    let finished =
        (*c.status() == ChainStatus::Done).then(|| outcome_of(c, input.verdicts, input.landings, input.owners));
    // The last verdict each card gave.
    let verdict_of: HashMap<&str, &Approval> = input.verdicts.iter().map(|a| (a.card_id.as_str(), a)).collect();
    let tree = tree_of(c, cards, input.edges);
    let by_id: HashMap<&str, &Card> = cards.iter().map(|k| (k.id.as_str(), k)).collect();
    let none: &[String] = &[];
    cards
        .iter()
        .map(|k| {
            let w = waiting.get(k.id.as_str()).map_or(&[][..], Vec::as_slice);
            let facts = CardFacts {
                waiting: w,
                last: input.last.get(&k.id),
                jobs: if k.id == c.chair_id { &submits } else { none },
                watches: input.watches.get(&k.id).map_or(none, Vec::as_slice),
                verdict: verdict_of.get(k.id.as_str()).copied(),
                finished: finished.as_ref(),
            };
            let (label, tone) = badge_of(k, c.status(), facts);
            let (doing, doing_tone) = doing_of(k, c, &input, facts);
            let on_message = input.on_message.get(&k.id).copied().filter(|_| in_turn(k));
            let paused = *c.status() == ChainStatus::Paused;
            let clock = clock_of(k, w, on_message, paused);
            let asks = asks_of(k, c.status());
            let stop_says = if asks.contains(&Ask::Stop) {
                chains::lifecycle::stop_says(k, Some(c))
            } else {
                String::new()
            };
            AgentStanding {
                card_id: k.id.clone(),
                home: c.id.clone(),
                label: label.to_owned(),
                tone,
                doing,
                doing_tone,
                work: clock.work,
                since: clock.since,
                warms: clock.warms,
                waiting: w.iter().map(|it| it.id.clone()).collect(),
                children: tree.children.get(&k.id).cloned().unwrap_or_default(),
                with: tree.with.get(&k.id).cloned().unwrap_or_default(),
                peers: if k.id == c.chair_id {
                    tree.peers.clone()
                } else {
                    Vec::new()
                },
                branch: branch_of(&k.id, &by_id, &tree.children),
                asks,
                stop_says,
                remembers: remembers(k),
                on_message,
                on_send: on_send_of(k, Some(c.status())),
                rev: 0,
            }
        })
        .collect()
}

/// The branch card id heads: its own turns and work, and those of everything that hangs under it.
fn branch_of(id: &str, by_id: &HashMap<&str, &Card>, children: &HashMap<String, Vec<String>>) -> Branch {
    let k = by_id.get(id);
    let mut b = Branch {
        agents: 0,
        turns: k.map_or(0, |k| k.turns),
        working: k.is_some_and(|k| at_work_now(k)),
    };
    for kid in children.get(id).map_or(&[][..], Vec::as_slice) {
        let kb = branch_of(kid, by_id, children);
        b.agents = b.agents.saturating_add(kb.agents.saturating_add(1));
        b.turns = b.turns.saturating_add(kb.turns);
        b.working = b.working || kb.working;
    }
    b
}

/// " · +n queued" when messages wait for k's next turn.
fn queued_of(k: &Card) -> String {
    if k.queued > 0 {
        format!(" · +{} queued", k.queued)
    } else {
        String::new()
    }
}

/// What a message the owner sends agent k now does, when it is not read at once: it waits for the turn in progress
/// (and those queued before it), for its session to open, or for the paused chain to run again; a waiting chain's Chair
/// is woken by it. chain is its chain's status, none for an agent in no chain. None when it is read now, or when k takes
/// no message (`asks_of`).
pub(crate) fn on_send_of(k: &Card, chain: Option<&ChainStatus>) -> Option<String> {
    if spawned(k) {
        return None;
    }
    match chain {
        Some(ChainStatus::Done) => return None,
        Some(ChainStatus::Paused) if k.origin == Origin::Chair => {
            return Some("The chain is paused: the Chair reads it first on Resume".to_owned());
        }
        Some(ChainStatus::Paused) => return Some("The chain is paused: it is read on Resume".to_owned()),
        Some(ChainStatus::Waiting) if k.origin == Origin::Chair && !at_work_now(k) => {
            return Some("Wakes the Chair now".to_owned());
        }
        _ => {}
    }
    let queued = if k.queued > 0 {
        format!(" (+{} queued)", k.queued)
    } else {
        String::new()
    };
    if in_turn(k) {
        return Some(format!("Waits for the current turn{queued}"));
    }
    if k.status() == &CardStatus::Starting {
        return Some(format!("Waits for its session to open{queued}"));
    }
    None
}

/// The first of items, open inbox items with one agent, in the board's order (the oldest: every kind of item holds the
/// agent up alike), as its line names it: "{label}: {ask}". It needs the owner only when it waits on him
/// (`waits_on_owner`); a failure is a failure whoever decides it, and the rest wait on the Assistant's answer.
fn first_wait(items: &[&InboxItem]) -> Option<(String, DoingTone)> {
    let first = items.iter().min_by_key(|it| (it.created_at, it.id.as_str()))?;
    let tone = if first.kind == InboxKind::Error {
        DoingTone::Error
    } else if waits_on_owner(first) {
        DoingTone::Needs
    } else {
        DoingTone::Waiting
    };
    Some((format!("{}: {}", label_of(kind_of(&first.kind)), first.title), tone))
}

/// Where an agent in no chain, card k (the Assistant), stands: it is in no tree, so its badge and its line say whether
/// it works, waits on the owner or is ready for him. `waiting` is its open items, `last` the newest thing it did while
/// running (store.LastDone), `on_message` the message it is on in a turn (store.LastTaken). The owner can
/// always write to it (a message opens a session when it has none), interrupt its turn, and start over once it has a
/// session.
pub(crate) fn standalone_standing(
    k: &Card,
    waiting: &[InboxItem],
    last: Option<&Event>,
    on_message: Option<OnMessage>,
    doing: &dyn Fn(&str) -> String,
) -> AgentStanding {
    let mut st = AgentStanding {
        card_id: k.id.clone(),
        home: k.home.clone(),
        waiting: waiting.iter().map(|it| it.id.clone()).collect(),
        branch: Branch {
            agents: 0,
            turns: k.turns,
            working: at_work_now(k),
        },
        asks: vec![Ask::Message],
        remembers: remembers(k),
        on_send: on_send_of(k, None),
        ..AgentStanding::default()
    };
    if in_turn(k) {
        st.asks.extend([Ask::Steer, Ask::Interrupt]);
        st.on_message = on_message;
    }
    if !k.session_id.is_empty() {
        st.asks.push(Ask::Restart);
    }
    let queued = queued_of(k);
    let items: Vec<&InboxItem> = waiting.iter().collect();
    let (label, tone, line, doing_tone) = if let Some((line, doing_tone)) = first_wait(&items) {
        let tone = if items.iter().any(|it| it.kind == InboxKind::Error) {
            AgentTone::Failed
        } else {
            AgentTone::Needs
        };
        ("Needs you", tone, line, doing_tone)
    } else if k.status() == &CardStatus::Failed {
        (
            "Error",
            AgentTone::Failed,
            format!("Error: {}", k.error),
            DoingTone::Error,
        )
    } else if k.status() == &CardStatus::Starting {
        (
            "Working",
            AgentTone::Running,
            format!("Opening a session…{queued}"),
            DoingTone::Busy,
        )
    } else if in_turn(k) {
        (
            "Working",
            AgentTone::Running,
            format!("{}{queued}", running_doing(last, &[], doing)),
            DoingTone::Busy,
        )
    } else {
        (
            "Ready",
            AgentTone::Idle,
            "Ready · ask anything".to_owned(),
            DoingTone::Idle,
        )
    };
    st.label = label.to_owned();
    st.tone = tone;
    st.doing = line;
    st.doing_tone = doing_tone;
    let clock = clock_of(k, &items, st.on_message, false);
    st.work = clock.work;
    st.since = clock.since;
    st.warms = clock.warms;
    st
}

/// What an agent's line says of its work and its time (`AgentStanding::work`, `since`, `warms`).
#[derive(Debug, Default, PartialEq, Eq)]
struct Clock {
    work: String,
    since: Option<Time>,
    warms: bool,
}

/// The work and time of agent k, items being what waits on the owner with it and on_message the message its turn is on:
/// a wait on the owner counts from the oldest item and never warms; a running turn, in a chain not paused, shows its
/// work and counts from the turn's start; every other state (a permission asked, a provider out, idle) has no clock.
fn clock_of(k: &Card, items: &[&InboxItem], on_message: Option<OnMessage>, paused: bool) -> Clock {
    if let Some(first) = items.iter().map(|it| it.created_at).min() {
        return Clock {
            since: Some(first),
            ..Clock::default()
        };
    }
    if paused || k.status() != &CardStatus::Running {
        return Clock::default();
    }
    Clock {
        work: work_of(k),
        since: on_message.map(|m| m.at),
        warms: true,
    }
}

/// Characters of a brief's first line that name its work.
const WORK_CHARS: usize = 80;

/// What agent k works on, in its own words: the step of its plan in progress, else its brief's first line (cut to
/// `WORK_CHARS`); empty when it holds neither.
pub(crate) fn work_of(k: &Card) -> String {
    if let Some(step) = k.plan.iter().find(|p| p.status == "in_progress") {
        return step.content.trim().to_owned();
    }
    let first = k
        .brief
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default();
    let first = first.trim_start_matches('#').trim();
    if first.chars().count() <= WORK_CHARS {
        return first.to_owned();
    }
    let cut: String = first.chars().take(WORK_CHARS).collect();
    format!("{}…", cut.trim_end())
}

/// An agent whose session is in a turn: working, or asking a permission in it.
pub(crate) fn in_turn(k: &Card) -> bool {
    matches!(k.status(), CardStatus::Running | CardStatus::Permission)
}

/// An agent whose session is in a turn (or opening one): its time keeps counting.
pub(crate) fn at_work_now(k: &Card) -> bool {
    matches!(
        k.status(),
        CardStatus::Running | CardStatus::Starting | CardStatus::Permission
    )
}

/// An agent's badge: what waits on the owner with it comes first (`waits_on_owner`), red when it is a failure; a
/// failure the Assistant decides for him is an error.
fn badge_of(k: &Card, chain: &ChainStatus, f: CardFacts<'_>) -> (&'static str, AgentTone) {
    let failed = f.waiting.iter().any(|i| i.kind == InboxKind::Error);
    if f.waiting.iter().any(|i| waits_on_owner(i)) {
        return ("Needs you", if failed { AgentTone::Failed } else { AgentTone::Needs });
    }
    if failed {
        return ("Error", AgentTone::Failed);
    }
    if !k.error.is_empty() || k.status() == &CardStatus::Failed {
        return ("Error", AgentTone::Failed);
    }
    if at_work_now(k) {
        return ("Running", AgentTone::Running);
    }
    if k.status() == &CardStatus::Waiting {
        return ("Waiting for a provider", AgentTone::Waiting);
    }
    if !f.jobs.is_empty() {
        return ("Submitting", AgentTone::Running);
    }
    if !f.watches.is_empty() {
        return ("Watching", AgentTone::Running);
    }
    if k.awaiting_review {
        return ("Awaiting acceptance", AgentTone::Waiting);
    }
    if let Some(e) = f.finished.filter(|_| k.origin == Origin::Chair) {
        return (e.label(), finished_tone(e).0);
    }
    if k.status() == &CardStatus::Stopped && spawned(k) {
        // a subagent that finishes ends idle with its report (spawn.rs's `spawn_end`): a stopped one was cut short
        return ("Stopped", AgentTone::Idle);
    }
    if k.status() == &CardStatus::Stopped && *chain == ChainStatus::Done {
        return ("Done", AgentTone::Done);
    }
    if k.status() == &CardStatus::Stopped {
        return ("Stopped", AgentTone::Done);
    }
    ("Idle", AgentTone::Idle)
}

/// The colour of a finished chain's Chair, by how the chain ended: Waiting to ship as waiting on an agent, Not done in no
/// colour of success.
fn finished_tone(e: &Ending) -> (AgentTone, DoingTone) {
    if e.done {
        (AgentTone::Done, DoingTone::Done)
    } else if e.shipping {
        (AgentTone::Waiting, DoingTone::Waiting)
    } else {
        (AgentTone::Idle, DoingTone::Idle)
    }
}

/// What agent k of chain c is doing, said plainly: the line the owner reads instead of guessing from a status.
fn doing_of(k: &Card, c: &Chain, input: &AgentsInput<'_>, f: CardFacts<'_>) -> (String, DoingTone) {
    let chain = c.status();
    // what waits on the owner with it comes first: the agent is held up on it
    if let Some(wait) = first_wait(f.waiting) {
        return wait;
    }
    if !k.error.is_empty() && k.status() != &CardStatus::Running {
        return (format!("Error: {}", k.error), DoingTone::Error);
    }
    if *chain == ChainStatus::Paused {
        return ("Chain is paused".to_owned(), DoingTone::Waiting);
    }
    // The wait is the Chair's: the others say where they stand.
    if *chain == ChainStatus::Waiting
        && k.origin == Origin::Chair
        && k.status() != &CardStatus::Running
        && k.status() != &CardStatus::Starting
    {
        let line = if !input.helping.is_empty() {
            // They are named; a check-back the chain also holds shows on the chain (wake_at).
            format!("Waiting on {}", input.helping.join(", "))
        } else if input.checks_back {
            "Chain is waiting to check back".to_owned()
        } else {
            "Chain is waiting".to_owned()
        };
        return (line, DoingTone::Waiting);
    }
    let queued = queued_of(k);
    match k.status() {
        CardStatus::Starting => return (format!("Opening session…{queued}"), DoingTone::Busy),
        // Its activity says which providers are out, and why.
        CardStatus::Waiting => return (k.activity.clone(), DoingTone::Waiting),
        CardStatus::Permission => {
            return match k.permission_wait() {
                // The Chair answers it, not the owner: at its step, and as its driver once the time limit passed. Its
                // activity names the request.
                Some(PermissionWait::Chair | PermissionWait::Driver { .. }) => (k.activity.clone(), DoingTone::Waiting),
                Some(PermissionWait::Owner) | None => ("Asks your permission".to_owned(), DoingTone::Needs),
            };
        }
        CardStatus::Running => {
            return (
                format!("{}{queued}", running_doing(f.last, input.cards, input.doing)),
                DoingTone::Busy,
            );
        }
        CardStatus::Stopped => {
            // A judge has no session to message.
            if let CardRole::Judge(kind) = CardRole::of(k) {
                let label = f.verdict.map(|v| (input.verdict_label)(v));
                return (judge_done(kind, label.as_deref()), DoingTone::Done);
            }
            if let Some(e) = f.finished.filter(|_| k.origin == Origin::Chair) {
                return (e.text.clone(), finished_tone(e).1);
            }
            if spawned(k) {
                // it ran inside its parent's process, which takes no message for it
                return ("Stopped before it reported".to_owned(), DoingTone::Idle);
            }
            if *chain == ChainStatus::Done {
                return ("Done · message to ask more".to_owned(), DoingTone::Done);
            }
            return ("Stopped · message to resume".to_owned(), DoingTone::Done);
        }
        CardStatus::Failed => return ("Error".to_owned(), DoingTone::Error),
        _ => {}
    }
    // Idle: between turns.
    if let Some(repo) = f.jobs.first() {
        return (format!("{}…", job_title(repo)), DoingTone::Busy);
    }
    if !f.watches.is_empty() {
        return (watching_line(f.watches), DoingTone::Busy);
    }
    if k.origin == Origin::Chair {
        return idle_chair_doing(c, input.cards, f.finished);
    }
    if k.awaiting_review {
        return (
            "Reported · waiting for the Chair to accept".to_owned(),
            DoingTone::Waiting,
        );
    }
    if !k.output.is_empty() {
        return ("Turn done · accepted by the Chair".to_owned(), DoingTone::Done);
    }
    ("Turn done".to_owned(), DoingTone::Done)
}

/// What the Chair of chain c does between its turns, nothing of it waiting on the owner: it waits for its agents (cards)
/// at work, for his acceptance of their reports, for the goal check, or for his direction.
fn idle_chair_doing(c: &Chain, cards: &[Card], finished: Option<&Ending>) -> (String, DoingTone) {
    // The judges answer Remit, not the Chair.
    let team = || cards.iter().filter(|w| w.on_team());
    let working: Vec<&str> = team().filter(|w| at_work_now(w)).map(|w| w.name.as_str()).collect();
    let unreviewed: Vec<&str> = team().filter(|w| w.awaiting_review).map(|w| w.name.as_str()).collect();
    if !working.is_empty() {
        return (
            format!("Waiting for {} to report", working.join(", ")),
            DoingTone::Waiting,
        );
    }
    if !unreviewed.is_empty() {
        return (
            format!("Needs your acceptance of the report from {}", unreviewed.join(", ")),
            DoingTone::Needs,
        );
    }
    if c.finishing.is_some() {
        return ("Waiting for the goal check".to_owned(), DoingTone::Waiting);
    }
    if let Some(e) = finished {
        return (e.text.clone(), finished_tone(e).1);
    }
    ("Idle · waiting for your direction".to_owned(), DoingTone::Idle)
}

/// What a running agent is doing, from the newest thing it did or was given to read: only the kind of tool, since its
/// card in the feed already shows the command or file. Its line says it after its work and its time (`clock_of`).
fn running_doing(last: Option<&Event>, cards: &[Card], doing: &dyn Fn(&str) -> String) -> String {
    let Some(last) = last else {
        return "Working…".to_owned();
    };
    match last.status {
        EventStatus::Tool(status) => {
            if status.ended() {
                return "Thinking about the next step…".to_owned();
            }
            if let Some(name) = last.title.strip_prefix(SOURCE_TOOL_PREFIX) {
                let d = doing(name);
                return if d.is_empty() { "Calling Remit…".to_owned() } else { d };
            }
            if let Some(d) = kind_doing(&last.tool_kind) {
                return d.to_owned();
            }
            match last.title.split_whitespace().next() {
                Some(name) => format!("Using {name}…"),
                None => "Using a tool…".to_owned(),
            }
        }
        EventStatus::Thought => "Thinking…".to_owned(),
        EventStatus::Message => "Replying…".to_owned(),
        EventStatus::Prompt(_) => {
            let (kind, who) = last
                .by
                .as_ref()
                .map_or((&ActorKind::Remit, ""), |by| (&by.kind, by.who.as_str()));
            match kind {
                ActorKind::Owner => return "Reading your message…".to_owned(),
                ActorKind::Assistant => return "Reading what the Assistant sent for you…".to_owned(),
                ActorKind::Client => return "Reading what an outside agent sent…".to_owned(),
                ActorKind::Remit if last.priority == PRIORITY_OPENING => return "Starting…".to_owned(),
                ActorKind::Remit => return "Reading what Remit says…".to_owned(),
                _ => {}
            }
            match cards.iter().find(|k| k.id == who) {
                None => "Reading a message…".to_owned(),
                Some(k) if k.origin == Origin::Chair => "Reading the Chair's direction…".to_owned(),
                Some(k) => format!("Reading what {} sent…", k.name),
            }
        }
        _ => "Working…".to_owned(),
    }
}

/// What an agent using one of its own tools is doing, by the tool's kind (agent.ToolCall's kind).
fn kind_doing(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "read" => "Reading a file…",
        "edit" => "Editing a file…",
        "delete" => "Deleting a file…",
        "move" => "Moving a file…",
        "search" => "Searching…",
        "execute" => "Running a command…",
        "fetch" => "Fetching a page…",
        "think" => "Thinking…",
        _ => return None,
    })
}

/// What an agent watches in the background: the oldest task, and how many more.
fn watching_line(watches: &[String]) -> String {
    let Some((first, rest)) = watches.split_first() else {
        return "Watching".to_owned();
    };
    if rest.is_empty() {
        format!("Watching: {first}")
    } else {
        format!("Watching: {first} (+{})", rest.len())
    }
}

/// What a judge of kind says once its session has ended: the goal judge, the verdict it gave (its label,
/// `verdict_check`'s; none when it gave none); another judge, what it did.
fn judge_done(kind: JudgeKind, verdict_label: Option<&str>) -> String {
    match kind {
        JudgeKind::Goal => verdict_label.unwrap_or("Gave no verdict"),
        JudgeKind::Duel => "Judged a duel",
        JudgeKind::Second => "Gave a second opinion",
        JudgeKind::Round => "Judged a round of attempts",
    }
    .to_owned()
}
