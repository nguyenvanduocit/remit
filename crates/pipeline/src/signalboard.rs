//! The Signals and Subscriptions tabs and the Assistant's read_signals show one thing, laid out here by the server
//! (AGENTS.md, Server-driven): what Remit waits for and what it does on its own, as the subscriptions of the signal
//! backbone (ADR-0014) that make it happen. Every subscription of every owner is a line, in the order the owner looks at
//! it, in the domain's words: a Chair's check-back or watch, a routine, and an event signal with the rules that place its
//! events. Beside the lines are the signals they listen to, each with its newest reading. The browser only draws it. A
//! subscription's readings and firings are read when it is opened (`signal_detail`), since a reading's value can be large
//! and the board is pushed whole.
//!
//! A reading's value, an event's included, came from outside Remit: it is shown as text, cut short, and never read as an
//! instruction by anyone.

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};
use std::sync::PoisonError;

use model::{
    ActionKind, Chain, ChainStatus, Control, EventSignal, Firing, FiringCause, FiringStatus, NearMiss, OwnerKind,
    Reading, SignalKey, Subscription, SubscriptionOwner, Time, Workflow, WorkflowRef, WorkflowStatus, event_key,
};
use time::macros::format_description;

use crate::Pipeline;
use crate::automation::{
    JOB_COLLECT_WORKTREES, JOB_EMBED_MEMORIES, JOB_FORGET, JOB_PACK_EVENTS, JOB_RECORD_LANDING_CHECKS,
    JOB_REVIEW_MEMORIES, JOB_SNAPSHOT, JOB_SYNC_LANDINGS, JOB_TEACH,
};
use crate::automation::{JOB_DIGEST_DAY, JOB_DIGEST_WEEK};
use crate::automation::{ROLE_GOAL_JUDGE, ROLE_GOAL_RECHECK};
use crate::event_record::event_ref;
use crate::forget::{STATE_READINGS_KEPT, STREAM_WINDOW};
use crate::record::hold_words;
use crate::routine::{cadence_label, is_routine, start_spec_of, unfinished_run};
use chains::admit::{Room, admit, why_not};
use chains::chain_core::first_line;
use chains::fact::keys;
use chains::signal_sources::{heard_on, signal_key_of, signal_source_of, signal_source_specs};
use chains::step::{ChainCmd, WakeCmd};
use chains::wait::{WakeSub, due_of, wait_owner, wakes_in};
use chains::watch::{NO_VALUE, cause_words};
use chains::{Error, Result};
use wire::app::{
    FiringLine, ReadingLine, SignalBoard, SignalDetail, SignalLine, SignalSource, SignalSourceLine, SilentSensor,
    SubscriptionGroup, SubscriptionKind, SubscriptionLine, SubscriptionState,
};

/// The most signals the board lists, the newest reading first; `signals_more` says how many more there are.
const SIGNALS_SHOWN: usize = 100;
/// How many of the newest readings, and of the newest firings, one opened subscription or signal gives.
const DETAIL_SHOWN: i64 = 20;
/// The longest value of a reading shown, in characters.
const VALUE_SHOWN: usize = 600;
/// How many of a subscription's newest near-misses its line shows; describe_subscription reads the rest.
const NEAR_MISSES_SHOWN: i64 = 3;

/// The group of a line in state st.
fn group_of(st: SubscriptionState) -> SubscriptionGroup {
    match st {
        SubscriptionState::Held | SubscriptionState::Due => SubscriptionGroup::Attention,
        SubscriptionState::Paused | SubscriptionState::Over => SubscriptionGroup::Idle,
        SubscriptionState::Waiting | SubscriptionState::Underway => SubscriptionGroup::Coming,
    }
}

/// What the board is laid out from.
#[derive(Debug, Clone, Default)]
pub(crate) struct BoardInput {
    pub chains: Vec<Chain>,
    /// Every subscription of every owner.
    pub subs: Vec<Subscription>,
    pub signals: Vec<EventSignal>,
    /// The newest reading of each signal, without its value.
    pub heads: Vec<Reading>,
    /// What admission weighs a wake and a start by.
    pub room: Room,
    pub silent: HashMap<SignalKey, String>,
    /// The silent sensors of each signal.
    pub silent_sensors: HashMap<SignalKey, Vec<SilentSensor>>,
    /// The firings recorded and not yet done, by subscription in the order they are delivered.
    pub pending: Vec<Firing>,
    /// The newest near-misses of each subscription, `NEAR_MISSES_SHOWN` of each, newest first.
    pub misses: Vec<NearMiss>,
    /// The workflow runs whose steps wait on a subscription, by id.
    pub runs: HashMap<String, Workflow>,
    pub now: Time,
}

impl BoardInput {
    /// The next firing to deliver of each subscription with firings still to be done, by subscription id: the first of
    /// its pending firings in the order they are delivered, the rest waiting behind it.
    fn next_firings(&self) -> HashMap<&str, &Firing> {
        let mut out = HashMap::new();
        for f in &self.pending {
            out.entry(f.subscription_id.as_str()).or_insert(f);
        }
        out
    }
}

type Heads<'a> = HashMap<&'a SignalKey, &'a Reading>;

/// Lays out in. It is pure: the Pipeline reads what it needs (`signal_board`) and a test hands it the records.
pub(crate) fn signal_board_of(input: &BoardInput) -> SignalBoard {
    let n = namer_of(&input.chains, &input.subs, &input.signals);
    let heads: Heads<'_> = input.heads.iter().map(|h| (&h.key, h)).collect();
    let mut lines = wake_lines(input, &heads);
    lines.extend(step_lines(input, &heads));
    lines.extend(routine_lines(input));
    lines.extend(automation_lines(input, &n, &heads));
    lines.extend(event_lines(input, &n, &heads));
    let held = with_lifecycle(&mut lines, input);
    sort_lines(&mut lines);
    let mut signals = signal_lines(input, &n, &heads, &lines);
    let sources = signal_source_lines(&lines, &signals);
    let more = signals.len().saturating_sub(SIGNALS_SHOWN);
    signals.truncate(SIGNALS_SHOWN);
    SignalBoard {
        sources,
        subscriptions: lines,
        signals,
        signals_more: more,
        held,
        rev: 0,
    }
}

/// The lines of the sources, each with how many of the board's lines and signals are its.
fn signal_source_lines(lines: &[SubscriptionLine], signals: &[SignalLine]) -> Vec<SignalSourceLine> {
    let mut out: Vec<SignalSourceLine> = signal_source_specs()
        .into_iter()
        .map(|s| SignalSourceLine {
            id: format!("source:{}", s.source.as_str()),
            source: s.source,
            title: s.title.to_owned(),
            pattern: s.pattern,
            what: s.what.to_owned(),
            who: s.who.to_owned(),
            ..SignalSourceLine::default()
        })
        .collect();
    for l in lines {
        if let Some(o) = out.iter_mut().find(|o| o.source == l.source) {
            o.subscriptions = o.subscriptions.saturating_add(1);
        }
    }
    for s in signals {
        if let Some(o) = out.iter_mut().find(|o| o.source == s.source) {
            o.signals = o.signals.saturating_add(1);
            o.last_read = o.last_read.max(s.at);
        }
    }
    out
}

/// Sets on each line where its subscription stands in the pipeline (docs/adr/0064): its control, its hold, what its next
/// firing's delivery waits on and its newest near-misses. It answers how many are held.
fn with_lifecycle(lines: &mut [SubscriptionLine], input: &BoardInput) -> usize {
    let subs: HashMap<&str, &Subscription> = input.subs.iter().map(|s| (s.id.as_str(), s)).collect();
    let mut misses: HashMap<&str, Vec<NearMiss>> = HashMap::new();
    for m in &input.misses {
        misses.entry(m.subscription_id.as_str()).or_default().push(m.clone());
    }
    let next = input.next_firings();
    let mut held = 0usize;
    for l in lines.iter_mut() {
        let Some(s) = subs.get(l.id.as_str()) else { continue };
        l.control = s.control;
        l.hold.clone_from(&s.hold);
        l.waiting = next.get(s.id.as_str()).map(|f| f.waiting.clone()).unwrap_or_default();
        l.near_misses = misses.remove(s.id.as_str()).unwrap_or_default();
        if s.hold.is_some() {
            held = held.saturating_add(1);
        }
    }
    held
}

/// The check-backs and watches of the chains that wait or are paused.
fn wake_lines(input: &BoardInput, heads: &Heads<'_>) -> Vec<SubscriptionLine> {
    let mut out = Vec::new();
    for c in &input.chains {
        if !matches!(c.status(), ChainStatus::Waiting | ChainStatus::Paused) {
            continue;
        }
        let owner = wait_owner(&c.id);
        let mine: Vec<Subscription> = input.subs.iter().filter(|s| s.owner == owner).cloned().collect();
        for sub in wakes_in(mine).0 {
            let head = heads.get(&SignalKey(sub.sub.r#match.0.clone())).copied();
            out.push(wake_line(c, &sub, input, head));
        }
    }
    out
}

/// The line of wake subscription sub of chain c. A watch whose source is silent is held, and says so. Admission weighs
/// one that is due only while its chain waits: a running chain holds its slot already.
fn wake_line(c: &Chain, w: &WakeSub, input: &BoardInput, head: Option<&Reading>) -> SubscriptionLine {
    let sub = &w.sub;
    let mut l = SubscriptionLine {
        id: sub.id.clone(),
        kind: SubscriptionKind::CheckBack,
        title: c.title.clone(),
        chain_id: c.id.clone(),
        source: signal_source_of(&heard_on(sub)),
        at: w.at,
        what: sub.action.why.clone(),
        last_fired: sub.last_fired,
        ..SubscriptionLine::default()
    };
    if sub.schedule.is_none() {
        l.kind = SubscriptionKind::Watch;
        l.key = sub.r#match.0.clone();
        l.condition.clone_from(&sub.expr);
        l.last_read = head.map(|h| h.at);
    }
    let silent = input.silent.get(&SignalKey(sub.r#match.0.clone()));
    if *c.status() == ChainStatus::Paused {
        l.state = SubscriptionState::Paused;
    } else if let Some(h) = &sub.hold {
        (l.state, l.held) = (SubscriptionState::Held, hold_words(h));
    } else if l.at.is_none_or(|at| input.now >= at) {
        l.state = SubscriptionState::Due;
        if *c.status() == ChainStatus::Waiting
            && let Err(err) = admit(c, &ChainCmd::Wake(WakeCmd::default()), &input.room)
        {
            (l.state, l.held) = (SubscriptionState::Held, why_not(&err));
        }
    } else if let Some(why) = silent.filter(|w| !w.is_empty()) {
        (l.state, l.held) = (SubscriptionState::Held, why.clone());
    }
    l
}

/// The waits of workflow runs (ADR-0073): each step of a run that waits for a time or a signal, as the check-back or the
/// watch of its chain it is, saying which run's step it resumes. A step waits whatever its chain does meanwhile (the
/// run's other cards may be at work), so it is a line as soon as it is made.
fn step_lines(input: &BoardInput, heads: &Heads<'_>) -> Vec<SubscriptionLine> {
    let chains: HashMap<&str, &Chain> = input.chains.iter().map(|c| (c.id.as_str(), c)).collect();
    let mut out = Vec::new();
    for s in &input.subs {
        let (Some(step), Some(c)) = (&s.action.workflow, chains.get(s.owner.id.as_str())) else {
            continue;
        };
        if s.action.kind != ActionKind::Workflow || s.owner.kind != OwnerKind::Chain {
            continue;
        }
        let run = input.runs.get(&step.run);
        let head = heads.get(&SignalKey(s.r#match.0.clone())).copied();
        let wake = WakeSub {
            sub: s.clone(),
            at: due_of(s),
        };
        let mut l = wake_line(c, &wake, input, head);
        l.what = step_words(s, step, run);
        if run.is_some_and(|r| r.status == WorkflowStatus::Paused) && s.hold.is_none() {
            (l.state, l.held) = (SubscriptionState::Paused, String::new());
        }
        out.push(l);
    }
    out
}

/// What a wait of a workflow run's step says to the owner: which step of which run it resumes, and why it waits. A run
/// that is gone is named by its id.
fn step_words(s: &Subscription, step: &WorkflowRef, run: Option<&Workflow>) -> String {
    let name = run
        .map(|r| r.name.as_str())
        .filter(|n| !n.is_empty())
        .unwrap_or(&step.run);
    let what = format!("A step of workflow {name} (step {})", step.seq);
    if s.action.why.is_empty() {
        return what;
    }
    format!("{what}: {}", s.action.why)
}

/// The owner's routines: each one's next run; held while no provider can run it or while its last run is at work; paused;
/// under way while a run is at work; over when it has no next time.
fn routine_lines(input: &BoardInput) -> Vec<SubscriptionLine> {
    input
        .subs
        .iter()
        .filter(|r| is_routine(r))
        .map(|r| routine_line(r, input))
        .collect()
}

/// The line of routine r. A run decided and not started yet (its firing recorded, not delivered) is due at the time it
/// was decided, or held with the reason its last attempt found, whatever its schedule says of the run after it.
fn routine_line(r: &Subscription, input: &BoardInput) -> SubscriptionLine {
    let next = input.next_firings();
    let f = next.get(r.id.as_str()).copied();
    let start = start_spec_of(r);
    let run = unfinished_run(&input.chains, &r.id);
    let at = f.map_or(r.due_at, |f| Some(f.at));
    let cron = r.schedule.as_ref().map(|s| s.cron.as_str()).unwrap_or_default();
    let mut l = SubscriptionLine {
        id: r.id.clone(),
        kind: SubscriptionKind::Routine,
        title: start.title.clone(),
        chain_id: run.map(|c| c.id.clone()).unwrap_or_default(),
        source: signal_source_of(&heard_on(r)),
        at,
        what: format!("{} · {}", first_line(&start.goal, 160), cadence_label(cron)),
        last_fired: r.last_fired,
        ..SubscriptionLine::default()
    };
    let held = held_why(r, f);
    if at.is_none() && run.is_some() {
        l.state = SubscriptionState::Underway;
    } else if at.is_none() || r.control == Control::Ended {
        l.state = SubscriptionState::Over;
    } else if r.control == Control::Paused {
        l.state = SubscriptionState::Paused;
    } else if !held.is_empty() {
        (l.state, l.held) = (SubscriptionState::Held, held);
    } else if f.is_some() || at.is_some_and(|at| input.now >= at) {
        l.state = SubscriptionState::Due;
        if !input.room.no_agents.is_empty() {
            (l.state, l.held) = (
                SubscriptionState::Held,
                why_not(&chains::admit::no_agents(&input.room.no_agents)),
            );
        } else if !input.room.no_headroom.is_empty() {
            (l.state, l.held) = (
                SubscriptionState::Held,
                why_not(&chains::admit::no_headroom(&input.room.no_headroom)),
            );
        } else if run.is_some() {
            l.state = SubscriptionState::Held;
            "its last run is still at work: the next starts once it is done".clone_into(&mut l.held);
        }
    }
    l
}

/// What Remit does on its own, one line for each subscription that hires, ends, runs, tells a deadline or assists.
fn automation_lines(input: &BoardInput, n: &Namer<'_>, heads: &Heads<'_>) -> Vec<SubscriptionLine> {
    let next = input.next_firings();
    let mut out = Vec::new();
    for s in &input.subs {
        let on_event = signal_source_of(&heard_on(s)) == SignalSource::Event;
        let kinds = [
            ActionKind::Hire,
            ActionKind::End,
            ActionKind::Run,
            ActionKind::Deadline,
            ActionKind::Assist,
        ];
        if s.owner.kind != OwnerKind::Remit || on_event || !kinds.contains(&s.action.kind) {
            continue;
        }
        let (title, what) = automation_words(s, n);
        let mut l = SubscriptionLine {
            id: s.id.clone(),
            kind: SubscriptionKind::Automation,
            title,
            source: signal_source_of(&heard_on(s)),
            at: due_of(s),
            what,
            last_fired: s.last_fired,
            ..SubscriptionLine::default()
        };
        if s.schedule.is_none() {
            l.key = s.r#match.0.clone();
            l.condition.clone_from(&s.expr);
            l.last_read = heads.get(&SignalKey(s.r#match.0.clone())).map(|h| h.at);
        }
        let held = held_why(s, next.get(s.id.as_str()).copied());
        if s.control == Control::Paused {
            l.state = SubscriptionState::Paused;
        } else if !held.is_empty() {
            (l.state, l.held) = (SubscriptionState::Held, held);
        } else if s.schedule.is_some() && s.due_at.is_some_and(|due| input.now >= due) {
            l.state = SubscriptionState::Due;
        }
        out.push(l);
    }
    out
}

/// The title and the words of what automation subscription s does, for the owner.
fn automation_words(s: &Subscription, n: &Namer<'_>) -> (String, String) {
    let words = |t: &str, w: &str| (t.to_owned(), w.to_owned());
    match s.action.kind {
        ActionKind::Hire => match s.action.role.as_str() {
            ROLE_GOAL_JUDGE => words(
                "Hire the goal judge",
                "When a Chair asks to finish, a judge outside the team's work checks the result against the goal.",
            ),
            ROLE_GOAL_RECHECK if s.r#match.0 == keys::chain_marked_live().0 => words(
                "Hire the goal judge again after a live mark",
                "When the owner marks a finished chain's change live and its last check left something unverified that a landing or a deploy settles, a judge looks again at what is live.",
            ),
            ROLE_GOAL_RECHECK => words(
                "Hire the goal judge again after a landing",
                "When the change of a finished chain lands and its last check left something unverified that a landing or a deploy settles, a judge looks again at what is live.",
            ),
            role => (format!("Hire {role}"), format!("Hires a {role} when its fact is recorded.")),
        },
        ActionKind::End => (format!("Goal check of {}", n.chain(&s.action.subject)), s.action.why.clone()),
        ActionKind::Deadline => (format!("A mentor's deadline in {}", n.chain(&s.owner.id)), s.action.why.clone()),
        ActionKind::Assist => words(
            "The Assistant decides first",
            "When an agent asks the owner (a question, a change item), the Assistant is told first and decides it for him, or leaves it to him.",
        ),
        ActionKind::Run => match s.action.job.as_str() {
            JOB_SYNC_LANDINGS => words(
                "Run the merge queues",
                "Runs every queue with a change ready to land, and has the forge read the checks of its pipeline on what landed until they settle (the subscription that records them is Record landing checks).",
            ),
            JOB_RECORD_LANDING_CHECKS => words(
                "Record landing checks",
                "Records the checks the forge's pipeline ran on what a chain landed, and reverts a landing whose commit failed where the base before it passed.",
            ),
            JOB_TEACH => (
                format!(
                    "Teach the employees · {}",
                    s.r#match.0.strip_prefix(model::FACT_KEY_PREFIX).unwrap_or(&s.r#match.0)
                ),
                "Records, for the librarian, what the agent's memory may learn from what it was taught: an accepted card's whole session, only the words of a judge who sent it back or a mentor who steered it, the owner's changes after its last push, and the owner's word where he overturned his Assistant (taking back what that answer taught).".to_owned(),
            ),
            JOB_EMBED_MEMORIES => words(
                "Embed memories",
                "Gives each memory with no vector its vector, so it is found by meaning as well as by its words: what was written while OpenRouter could not be reached.",
            ),
            JOB_REVIEW_MEMORIES => words(
                "Review memories",
                "Asks the librarian to join memories that say the same thing, settle contradictions both sides of which stand, and correct or retract what misled the agents that used it.",
            ),
            JOB_COLLECT_WORKTREES => words(
                "Collect worktrees",
                "Removes the worktrees of finished chains with nothing left to review, once they are past what the server keeps.",
            ),
            JOB_SNAPSHOT => words(
                "Snapshot the database",
                "Copies the database when the last scheduled snapshot is old enough.",
            ),
            JOB_PACK_EVENTS => words(
                "Pack settled events",
                "Packs the events of settled chains so the database stays small.",
            ),
            JOB_FORGET => {
                let days = STREAM_WINDOW.whole_days();
                (
                    "Forget old readings".to_owned(),
                    format!(
                        "Forgets the readings no subscription still has to judge: an event's or a fact's after {days} days, a state's beyond its newest {STATE_READINGS_KEPT}, never the newest of a signal; and the firings done after {days} days."
                    ),
                )
            }
            JOB_DIGEST_DAY | JOB_DIGEST_WEEK => words(
                "Post the digest",
                "Posts in the Assistant's panel, at the time the owner told it, what happened over the last day or week: the chains that finished and how each ended, what the Assistant decided for him and what he overturned. Remit writes it from what it recorded, with no model call.",
            ),
            job => (format!("Run {job}"), format!("Runs the job {job} on its schedule.")),
        },
        _ => (s.action.kind.as_str().to_owned(), String::new()),
    }
}

/// The subscriptions to the readings of an event signal, one line for each, whoever made it.
fn event_lines(input: &BoardInput, n: &Namer<'_>, heads: &Heads<'_>) -> Vec<SubscriptionLine> {
    let next = input.next_firings();
    input
        .subs
        .iter()
        .filter(|s| signal_source_of(&SignalKey(s.r#match.0.clone())) == SignalSource::Event)
        .map(|s| {
            let head = heads.get(&SignalKey(s.r#match.0.clone())).copied();
            event_line(s, n, head, held_why(s, next.get(s.id.as_str()).copied()))
        })
        .collect()
}

/// The line of subscription s to the events of a signal: paused when its owner or its event signal is, and held, with the
/// reason, while a firing failed for good or waits.
fn event_line(s: &Subscription, n: &Namer<'_>, head: Option<&Reading>, held: String) -> SubscriptionLine {
    let key = SignalKey(s.r#match.0.clone());
    let name = signal_key_of(&key).map(|k| k.name).unwrap_or_default();
    let es = n.signals.get(name).copied();
    let (title, what) = event_words(s, name);
    let mut l = SubscriptionLine {
        id: s.id.clone(),
        kind: SubscriptionKind::Event,
        title,
        event_signal_id: es.map(|e| e.id.clone()).unwrap_or_default(),
        source: SignalSource::Event,
        key: s.r#match.0.clone(),
        condition: s.expr.clone(),
        screen: s.screen.clone(),
        what,
        last_fired: s.last_fired,
        last_read: head.map(|h| h.at),
        ..SubscriptionLine::default()
    };
    if s.control == Control::Paused || es.is_some_and(|e| e.paused) {
        l.state = SubscriptionState::Paused;
    } else if !held.is_empty() {
        (l.state, l.held) = (SubscriptionState::Held, held);
    }
    l
}

/// The title and the words of what subscription s to the events of signal name does. Its title is its note, with the
/// signal it listens to; one with no note is titled by what its kind does.
pub(crate) fn event_words(s: &Subscription, name: &str) -> (String, String) {
    let (title, what) = event_kind_words(s, name);
    if s.note.is_empty() {
        return (title, what);
    }
    (
        format!("{} · {name}", s.note.strip_suffix('.').unwrap_or(&s.note)),
        what,
    )
}

/// The title and the words of what a subscription of s's kind to the events of signal name does.
fn event_kind_words(s: &Subscription, name: &str) -> (String, String) {
    match s.action.kind {
        ActionKind::Tell => (
            format!("Tell each {name} event to its chain's Chair"),
            "Tells the Chair of the chain an event is about, once for each event it holds for.".to_owned(),
        ),
        ActionKind::Start => (
            format!("Start a chain · {name}: {}", start_spec_of(s).title),
            "Starts a chain for each event it holds for.".to_owned(),
        ),
        ActionKind::Run => (
            format!("Run {} · {name}", s.action.job),
            format!("Runs the job {} for each event it holds for.", s.action.job),
        ),
        _ => (format!("{} · {name}", s.action.kind.as_str()), String::new()),
    }
}

/// The address of the line of the signal key.
pub(crate) fn signal_address(key: &SignalKey) -> String {
    format!("signal:{}", key.0)
}

/// Puts the lines in the order the owner looks at them: what is held or due, then what comes next, then what is paused
/// or over. Within a rank, what waits for a time comes soonest first, then what waits for a signal, the newest reading
/// first.
fn sort_lines(lines: &mut [SubscriptionLine]) {
    for l in lines.iter_mut() {
        l.group = group_of(l.state);
    }
    let by_time = |l: &SubscriptionLine| !matches!(l.kind, SubscriptionKind::Watch | SubscriptionKind::Event);
    lines.sort_by(|a, b| {
        a.group.cmp(&b.group).then_with(|| match (by_time(a), by_time(b)) {
            (true, true) => a.at.cmp(&b.at),
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => b.last_read.cmp(&a.last_read).then_with(|| a.at.cmp(&b.at)),
        })
    });
}

/// The signals with a reading or a subscription, and every event signal the owner opened, the newest reading first and
/// those with none after them. Each counts the lines that listen to it.
fn signal_lines(input: &BoardInput, n: &Namer<'_>, heads: &Heads<'_>, subs: &[SubscriptionLine]) -> Vec<SignalLine> {
    let mut listeners: HashMap<&str, usize> = HashMap::new();
    for l in subs.iter().filter(|l| !l.key.is_empty()) {
        let c = listeners.entry(l.key.as_str()).or_default();
        *c = c.saturating_add(1);
    }
    let mut keys: BTreeSet<SignalKey> = heads.keys().map(|k| (*k).clone()).collect();
    // time is no signal to look at: a schedule's instants are only what they fired
    keys.extend(
        input
            .subs
            .iter()
            .filter(|s| s.schedule.is_none())
            .map(|s| SignalKey(s.r#match.0.clone())),
    );
    keys.extend(n.signals.keys().map(|name| event_key(name)));
    let mut lines: Vec<SignalLine> = keys
        .into_iter()
        .map(|k| {
            let h = heads.get(&k).copied();
            let event_signal_id = signal_key_of(&k)
                .filter(|kk| kk.source == SignalSource::Event)
                .and_then(|kk| n.signals.get(kk.name))
                .map(|es| es.id.clone())
                .unwrap_or_default();
            SignalLine {
                id: signal_address(&k),
                key: k.0.clone(),
                source: signal_source_of(&k),
                title: n.signal(&k),
                rev: h.map_or(0, |h| h.rev),
                at: h.map(|h| h.at),
                cause: h
                    .filter(|h| h.rev > 0)
                    .map(|h| cause_words(&h.cause.actor))
                    .unwrap_or_default(),
                subscribers: listeners.get(k.0.as_str()).copied().unwrap_or_default(),
                silent: input.silent.get(&k).cloned().unwrap_or_default(),
                silent_sensors: input.silent_sensors.get(&k).cloned().unwrap_or_default(),
                event_signal_id,
            }
        })
        .collect();
    lines.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| a.key.cmp(&b.key)));
    lines
}

/// Firings f in words.
pub(crate) fn firing_lines(f: &[Firing], n: &Namer<'_>) -> Vec<FiringLine> {
    f.iter()
        .map(|x| FiringLine {
            subscription_id: x.subscription_id.clone(),
            chain_id: x.chain.clone(),
            card_id: x.card.clone(),
            at: x.at,
            title: n.owner(&x.owner, &x.subscription_id),
            key: if signal_source_of(&x.key) == SignalSource::Clock {
                String::new()
            } else {
                x.key.0.clone()
            },
            action: x.action.clone(),
            outcome: firing_outcome(x),
            expired: x.cause == FiringCause::Expired,
        })
        .collect()
}

/// What became of firing f, in words: what it did once done, and otherwise where it stands and why.
pub(crate) fn firing_outcome(f: &Firing) -> String {
    match f.status {
        FiringStatus::Pending if !f.waiting.is_empty() => format!("waits: {}", f.waiting),
        FiringStatus::Pending if f.last_error.is_empty() => "to be done".to_owned(),
        FiringStatus::Pending => format!(
            "attempt {} failed ({}): tried again at {}",
            f.attempts,
            f.last_error,
            f.next_attempt
                .and_then(|at| at.0.format(format_description!("[hour]:[minute]:[second]")).ok())
                .unwrap_or_default()
        ),
        FiringStatus::Failed => format!("failed after {} attempts: {}", f.attempts, f.last_error),
        FiringStatus::Skipped => format!("skipped: given up after it failed ({})", f.last_error),
        FiringStatus::Cancelled => "cancelled: never to be done".to_owned(),
        _ => f.outcome.clone(),
    }
}

/// Why s does not fire though it would: the firing it is held on, else what its next firing to deliver waits for (next,
/// none when none is recorded); empty when nothing holds it. A firing that failed an attempt and waits out its backoff
/// holds nothing: it is tried again on its own, and its row says why it failed.
fn held_why(s: &Subscription, next: Option<&Firing>) -> String {
    if let Some(h) = &s.hold {
        return hold_words(h);
    }
    next.filter(|f| !f.waiting.is_empty())
        .map(firing_outcome)
        .unwrap_or_default()
}

/// Gives the records the owner knows by name their names.
pub(crate) struct Namer<'a> {
    /// A chain's title, by id.
    chains: HashMap<&'a str, &'a str>,
    /// A routine's or an event subscription's title, by subscription id.
    titles: HashMap<&'a str, String>,
    /// By name.
    signals: HashMap<&'a str, &'a EventSignal>,
}

pub(crate) fn namer_of<'a>(chains: &'a [Chain], subs: &'a [Subscription], signals: &'a [EventSignal]) -> Namer<'a> {
    let mut titles = HashMap::new();
    for s in subs {
        let key = SignalKey(s.r#match.0.clone());
        if let Some(k) = signal_key_of(&key).filter(|k| k.source == SignalSource::Event) {
            titles.insert(s.id.as_str(), event_words(s, k.name).0);
        } else if is_routine(s) {
            titles.insert(s.id.as_str(), start_spec_of(s).title);
        }
    }
    Namer {
        chains: chains.iter().map(|c| (c.id.as_str(), c.title.as_str())).collect(),
        titles,
        signals: signals.iter().map(|es| (es.name.as_str(), es)).collect(),
    }
}

impl Namer<'_> {
    /// The title of chain id, or its id when the chain is gone.
    fn chain(&self, id: &str) -> String {
        self.chains
            .get(id)
            .filter(|t| !t.is_empty())
            .map_or(id, |t| t)
            .to_owned()
    }

    /// Whose subscription subscription fired or is, owner being its owner.
    fn owner(&self, owner: &SubscriptionOwner, subscription: &str) -> String {
        if owner.kind == OwnerKind::Chain {
            return self.chain(&owner.id);
        }
        self.titles
            .get(subscription)
            .filter(|t| !t.is_empty())
            .cloned()
            .unwrap_or_else(|| "Remit".to_owned())
    }

    /// What key is in words: the event signal, the fact, or the record of a chain.
    fn signal(&self, key: &SignalKey) -> String {
        let Some(k) = signal_key_of(key) else {
            return key.0.clone();
        };
        match k.source {
            SignalSource::Event => format!("Events of {}", k.name),
            SignalSource::Landing => format!("Landing of {:?} in {}", self.chain(k.chain_id), k.name),
            SignalSource::Live => format!("{:?} is live", self.chain(k.chain_id)),
            SignalSource::Sensor => format!("Sensor {} of {:?}", k.name, self.chain(k.chain_id)),
            SignalSource::Fact => format!("Fact {}", k.name),
            SignalSource::Clock | SignalSource::None => key.0.clone(),
        }
    }
}

/// A reading's value as the text the owner reads, at most `VALUE_SHOWN` characters, and whether it was cut. The value is
/// shown as it came: no part of it is read as anything but text.
fn value_text(v: Option<&model::ForeignJson>) -> (String, bool) {
    let text = v.map(model::ForeignJson::text).unwrap_or_default();
    let s = text.trim();
    if s.is_empty() {
        return (NO_VALUE.to_owned(), false);
    }
    match s.char_indices().nth(VALUE_SHOWN) {
        None => (s.to_owned(), false),
        Some((at, _)) => (s.get(..at).unwrap_or(s).to_owned(), true),
    }
}

/// Reading r as the owner reads it: its value as text and, for an event, the address of what became of it.
pub(crate) fn reading_line(r: &Reading) -> ReadingLine {
    let (value, truncated) = value_text(r.value.as_ref());
    let event_id = if signal_source_of(&r.key) == SignalSource::Event {
        event_ref(r)
    } else {
        String::new()
    };
    ReadingLine {
        rev: r.rev,
        at: r.at,
        cause: cause_words(&r.cause.actor),
        value,
        truncated,
        event_id,
    }
}

impl Pipeline {
    /// Reads what the board is laid out from and lays it out.
    pub(crate) async fn signal_board(&self, chains: Vec<Chain>, room: Room, now: Time) -> Result<SignalBoard> {
        let (subs, signals, heads, pending, misses) = self
            .store
            .read(|r| {
                Ok::<_, store::Error>((
                    r.subscriptions()?,
                    r.event_signals()?,
                    r.signal_heads()?,
                    r.pending_firings("")?,
                    r.recent_near_misses(NEAR_MISSES_SHOWN)?,
                ))
            })
            .await
            .map_err(|e| Error::refused(format!("the signal board: {e}")))?;
        let runs = self.waiting_runs(&subs).await?;
        let (silent, silent_sensors) = self.sensor_silence().await;
        Ok(signal_board_of(&BoardInput {
            chains,
            subs,
            signals,
            heads,
            room,
            silent,
            silent_sensors,
            pending,
            misses,
            runs,
            now,
        }))
    }

    /// Reads the workflow runs whose steps wait on one of subs, by id: what a step's line names. A run that is gone
    /// leaves its line named by its id.
    async fn waiting_runs(&self, subs: &[Subscription]) -> Result<HashMap<String, Workflow>> {
        let ids: BTreeSet<String> = subs
            .iter()
            .filter(|s| s.action.kind == ActionKind::Workflow)
            .filter_map(|s| s.action.workflow.as_ref().map(|w| w.run.clone()))
            .collect();
        self.store
            .read(move |r| {
                let mut runs = HashMap::new();
                for id in ids {
                    match r.workflow(&id) {
                        Ok(run) => {
                            runs.insert(id, run);
                        }
                        Err(store::Error::NotFound) => {}
                        Err(err) => return Err(err),
                    }
                }
                Ok(runs)
            })
            .await
            .map_err(|e| Error::refused(format!("the signal board's workflow runs: {e}")))
    }

    /// Lays the board out for the Tick from chains, as its starts left them, keeps it, and answers why each line that is
    /// held is held (the idle watchdog's reasons). A board that cannot be read is logged and the last one stays.
    pub async fn lay_out_board(&self, chains: &[Chain], now: Time) -> Vec<String> {
        let running = chains.iter().filter(|c| *c.status() == ChainStatus::Running).count();
        let cap = self.capacity();
        let room = Room {
            no_agents: self.no_agents(),
            no_headroom: self.no_headroom(),
            limit: chains::admit::Pool {
                busy: running,
                most: cap.limit.unwrap_or(1),
            },
            pressure: cap.pressure,
            ..Room::default()
        };
        match self.signal_board(chains.to_vec(), room, now).await {
            Ok(board) => {
                let held = board.subscriptions.iter().map(|l| l.held.clone()).collect();
                self.keep_board(board);
                held
            }
            Err(err) => {
                tracing::error!(%err, "the signal board");
                Vec::new()
            }
        }
    }

    /// The board as the last Tick laid it out.
    pub fn signal_board_now(&self) -> SignalBoard {
        self.signals
            .board
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Makes next the board and tells the channels when it changed. An unchanged board is not told again, and keeps its
    /// revision.
    pub(crate) fn keep_board(&self, mut next: SignalBoard) {
        let kept = {
            let mut board = self.signals.board.lock().unwrap_or_else(PoisonError::into_inner);
            next.rev = board.rev;
            if *board == next {
                return;
            }
            next.rev = board.rev.saturating_add(1);
            *board = next;
            board.clone()
        };
        self.notify.signal_board_updated(&kept);
    }

    /// What the line at address adds to the board: the newest readings of its signal and the firings of its subscription,
    /// or for a signal the firings on it. The address is one the board gave.
    pub async fn signal_detail(&self, address: &str) -> Result<SignalDetail> {
        let (chains, subs, signals) = self
            .store
            .read(|r| Ok::<_, store::Error>((r.chains()?, r.subscriptions()?, r.event_signals()?)))
            .await?;
        let n = namer_of(&chains, &subs, &signals);
        let (key, title, filter) = if let Some(k) = address.strip_prefix("signal:") {
            let key = SignalKey(k.to_owned());
            let title = n.signal(&key);
            (
                key.clone(),
                title,
                store::FiringFilter {
                    key,
                    ..store::FiringFilter::default()
                },
            )
        } else {
            let Some(s) = subs.iter().find(|s| s.id == address) else {
                return Err(Error::refused(format!(
                    "no subscription {address:?}: it may have fired and ended; read_signals shows the ones there are"
                )));
            };
            let filter = store::FiringFilter {
                subscription: s.id.clone(),
                ..store::FiringFilter::default()
            };
            (heard_on(s), n.owner(&s.owner, &s.id), filter)
        };
        let mut d = SignalDetail {
            title,
            ..SignalDetail::default()
        };
        if signal_source_of(&key) != SignalSource::Clock {
            d.key = key.0.clone();
            let rs = self.store.read(move |r| r.newest_readings(&key, DETAIL_SHOWN)).await?;
            d.readings = rs.iter().map(reading_line).collect();
        }
        let fired = self.store.read(move |r| r.firings(&filter, DETAIL_SHOWN)).await?;
        d.firings = firing_lines(&fired, &n);
        Ok(d)
    }
}
