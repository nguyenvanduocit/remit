//! `Broadcast` is the channels' `orch::Notifier`: each change is turned into its contract push once and queued for every
//! open session it reaches, which is pushed its changes in the order they were stored. Every push names its audience
//! (`Audience`): the owner's session is pushed all but a mentor's own drafts; a mentor's session (ADR-0089) only what is
//! addressed to the chains and cards of their helps or to them, with where an agent and a chain stand as they stand for
//! them, and their board laid out again whenever a help of theirs moves. The pushes of a home that stream (its events and
//! what its agents show of the moment) reach only the sessions that show that home (`Shown`, `homes_shown`), so an open
//! session costs what it shows, not every chain's every chunk (ADR-0095). What waits to be sent is held once per record
//! (`Pending`): a slow session is sent each record's last state, not every step to it. A session with `BEHIND_BY`
//! records waiting is closed rather than left to miss one: its client connects again and reloads what it shows. How fast
//! the peer reads is the link's to watch (`link::Config::behind_bytes`); this bound only keeps the queue in front of it.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};

use model::{Card, ClientKey, Decision, Draft, Edge, Employee, Event, Help, InboxItem, WorkerKey};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use wire::app::ChainRead;
use wire::app::{
    AgentGone, ChainGone, ClientKeyGone, DraftGone, EventGone, EventSignalGone, ExtensionGone, HomeShown, PlaybookGone,
    Push, SensorGone, SkillGone,
};

use crate::pending::Pending;

/// How many records a session may have waiting to be sent before it counts as one that cannot keep up.
pub const BEHIND_BY: usize = 512;

/// Every open session's queue of changes.
pub struct Broadcast {
    me: Weak<Broadcast>,
    subs: Mutex<Subs>,
}

#[derive(Default)]
struct Subs {
    next: u64,
    by_id: HashMap<u64, Sub>,
}

struct Sub {
    outbox: Arc<Outbox>,
    behind: CancellationToken,
    seat: Seat,
    shown: Shown,
}

impl Drop for Sub {
    fn drop(&mut self) {
        self.outbox.close();
    }
}

/// One session's pushes waiting to be sent, and the wake of the loop that sends them.
#[derive(Default)]
struct Outbox {
    pending: Mutex<Pending>,
    ready: Notify,
    closed: AtomicBool,
}

impl Outbox {
    /// A section under this lock leaves the queue whole whatever panics, so a poisoned lock is taken as it is.
    fn pending(&self) -> MutexGuard<'_, Pending> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.ready.notify_one();
    }
}

/// The homes a session shows, which the pushes of a home that stream reach.
enum Shown {
    /// It never named any (a channel that reads every push, as `remit-client watch` does).
    Every,
    /// The homes of its telling of `seq`, the latest applied.
    Homes { homes: HashSet<String>, seq: u64 },
}

impl Shown {
    fn reached_by(&self, push: &Push) -> bool {
        match (self, push.streamed_home()) {
            (Shown::Homes { homes, .. }, Some(home)) => homes.contains(home),
            (Shown::Every, _) | (Shown::Homes { .. }, None) => true,
        }
    }

    /// Whether the telling of seq comes after the one applied: a session's calls are served at once, so an older
    /// telling, or one replayed, can arrive after a newer one, and would undo it.
    fn follows(&self, seq: u64) -> bool {
        match self {
            Shown::Every => true,
            Shown::Homes { seq: applied, .. } => seq > *applied,
        }
    }
}

/// Who a session signed in as, as the broadcast reaches them.
enum Seat {
    Owner,
    Mentor(MentorSeat),
}

/// A mentor's session: their key, and their helps as last pushed, which decide what reaches them.
struct MentorSeat {
    key: WorkerKey,
    helps: Vec<Help>,
}

impl MentorSeat {
    /// Keeps help h when it is theirs and newer than the copy held; whether it was kept.
    fn learn(&mut self, h: &Help) -> bool {
        if h.mentor != self.key.mentor {
            return false;
        }
        match self.helps.iter_mut().find(|held| held.id == h.id) {
            Some(held) if held.rev >= h.rev => false,
            Some(held) => {
                held.clone_from(h);
                true
            }
            None => {
                self.helps.push(h.clone());
                true
            }
        }
    }

    fn helps_card(&self, card_id: &str) -> bool {
        self.helps.iter().any(|h| h.card == card_id)
    }

    /// Whether a push addressed to audience reaches this mentor.
    fn reached_by(&self, audience: &Audience<'_>) -> bool {
        match audience {
            Audience::Owner => false,
            Audience::Chain(id) => self.helps.iter().any(|h| h.chain == *id),
            Audience::Card(id) => self.helps_card(id),
            Audience::Session(id) => self.helps.iter().any(|h| h.card == *id && orch::mentor_reads(h)),
            Audience::Writer(mentor) => *mentor == self.key.mentor,
        }
    }
}

/// Whom a push is addressed to: the owner alone, everyone on a chain or a card, everyone reading a card's session, or one
/// draft's writer (a mentor's id; empty: the owner).
enum Audience<'a> {
    Owner,
    Chain(&'a str),
    Card(&'a str),
    Session(&'a str),
    Writer(&'a str),
}

impl Audience<'_> {
    fn reaches_owner(&self) -> bool {
        !matches!(self, Audience::Writer(mentor) if !mentor.is_empty())
    }
}

/// Holds push for a session to be sent: false when the session is to be dropped, too far behind and told so.
fn queue(s: &Sub, push: Push) -> bool {
    let (waiting, reset) = {
        let mut pending = s.outbox.pending();
        pending.put(push);
        let reset = (pending.len() > BEHIND_BY)
            .then(|| pending.reset_busiest_home())
            .flatten();
        (pending.len(), reset)
    };
    if let Some(home) = reset {
        tracing::info!(
            home,
            "a session fell behind on a home: what streamed there is let go and the home read again"
        );
    }
    if waiting > BEHIND_BY {
        tracing::warn!(
            behind = BEHIND_BY,
            "a session fell behind: closed, it reloads when it connects again"
        );
        s.behind.cancel();
        return false;
    }
    s.outbox.ready.notify_one();
    true
}

/// One session's changes from the moment it subscribed, until it is dropped. `behind` is cancelled when the session fell
/// too far behind and was dropped from the broadcast.
pub(crate) struct Subscription {
    id: u64,
    from: Weak<Broadcast>,
    outbox: Arc<Outbox>,
    pub(crate) behind: CancellationToken,
}

/// Sets the homes one session shows: its calls' handler holds it.
#[derive(Clone)]
pub(crate) struct Showing {
    id: u64,
    from: Weak<Broadcast>,
}

impl Showing {
    /// From now on the session is pushed what streams in these homes and in no other, and what waits for any other is
    /// let go. Each home is then pushed `home_shown` with seq, after all that was held for it: the session's core reads a
    /// home it names anew from that mark, as what streamed there before it may predate the naming (ADR-0095 decision 7).
    /// A telling older than the one applied changes nothing. A session the marks put too far behind is dropped, as any
    /// push would.
    pub(crate) fn set(&self, homes: Vec<String>, seq: u64) {
        let Some(b) = self.from.upgrade() else { return };
        let mut subs = b.subs();
        let Some(sub) = subs.by_id.get_mut(&self.id).filter(|sub| sub.shown.follows(seq)) else {
            return;
        };
        let homes: HashSet<String> = homes.into_iter().collect();
        sub.outbox.pending().keep_homes(&homes);
        let marked = homes.iter().all(|home| {
            let mark = HomeShown {
                home: home.clone(),
                seq,
            };
            queue(sub, Push::home_shown { mark })
        });
        sub.shown = Shown::Homes { homes, seq };
        if !marked {
            subs.by_id.remove(&self.id);
        }
    }
}

impl Subscription {
    /// The next push to send, as soon as one waits; none once the broadcast dropped the session.
    pub(crate) async fn next(&self) -> Option<Push> {
        loop {
            if let Some(push) = self.outbox.pending().take() {
                return Some(push);
            }
            if self.outbox.closed.load(Ordering::Acquire) {
                return None;
            }
            self.outbox.ready.notified().await;
        }
    }

    /// Sets the homes this session shows.
    pub(crate) fn showing(&self) -> Showing {
        Showing {
            id: self.id,
            from: self.from.clone(),
        }
    }

    /// The mentor's helps as read once the session subscribed: each kept unless a newer copy was pushed meanwhile.
    pub(crate) fn seed(&self, helps: &[Help]) {
        if let Some(b) = self.from.upgrade()
            && let Some(Sub {
                seat: Seat::Mentor(seat),
                ..
            }) = b.subs().by_id.get_mut(&self.id)
        {
            for h in helps {
                seat.learn(h);
            }
        }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(b) = self.from.upgrade() {
            b.subs().by_id.remove(&self.id);
        }
    }
}

impl Broadcast {
    pub fn new() -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            me: me.clone(),
            subs: Mutex::default(),
        })
    }

    /// A section under this lock leaves the sessions whole whatever panics, so a poisoned lock is taken as it is.
    fn subs(&self) -> MutexGuard<'_, Subs> {
        self.subs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Queues every change that reaches the owner from now on for his session.
    pub(crate) fn subscribe(&self) -> Subscription {
        self.join(Seat::Owner)
    }

    /// Queues every change that reaches the mentor of key from now on for their session; `Subscription::seed` gives it
    /// their helps.
    pub(crate) fn subscribe_mentor(&self, key: WorkerKey) -> Subscription {
        self.join(Seat::Mentor(MentorSeat { key, helps: Vec::new() }))
    }

    fn join(&self, seat: Seat) -> Subscription {
        let outbox = Arc::new(Outbox::default());
        let behind = CancellationToken::new();
        let mut subs = self.subs();
        let id = subs.next;
        subs.next = subs.next.wrapping_add(1);
        subs.by_id.insert(
            id,
            Sub {
                outbox: Arc::clone(&outbox),
                behind: behind.clone(),
                seat,
                shown: Shown::Every,
            },
        );
        Subscription {
            id,
            from: self.me.clone(),
            outbox,
            behind,
        }
    }

    /// How many sessions are subscribed.
    pub fn sessions(&self) -> usize {
        self.subs().by_id.len()
    }

    /// Queues push for every session audience reaches; one whose queue is full is dropped and told it fell behind,
    /// without waiting for it.
    fn send(&self, audience: &Audience<'_>, push: &Push) {
        self.send_with(audience, push, |_| Some(push.clone()));
    }

    /// `send`, a mentor's session queued what mentor makes of push for them (none: nothing).
    fn send_with(&self, audience: &Audience<'_>, push: &Push, mentor: impl Fn(&[Help]) -> Option<Push>) {
        self.subs().by_id.retain(|_, s| match &s.seat {
            _ if !s.shown.reached_by(push) => true,
            Seat::Owner if audience.reaches_owner() => queue(s, push.clone()),
            Seat::Mentor(m) if m.reached_by(audience) => mentor(&m.helps).is_none_or(|p| queue(s, p)),
            Seat::Owner | Seat::Mentor(_) => true,
        });
    }
}

impl orch::chains::Notifier for Broadcast {
    fn chain_updated(&self, c: &ChainRead) {
        let chain = c.clone();
        self.send(&Audience::Chain(&c.chain.id), &Push::chain_updated { chain });
    }

    fn chain_deleted(&self, chain_id: &str) {
        let gone = ChainGone {
            id: chain_id.to_owned(),
        };
        self.send(&Audience::Chain(chain_id), &Push::chain_deleted { gone });
    }

    fn card_updated(&self, c: &Card) {
        let card = c.clone();
        self.send(&Audience::Card(&c.id), &Push::card_updated { card });
    }

    fn edge_added(&self, e: &Edge) {
        let edge = e.clone();
        self.send(&Audience::Owner, &Push::edge_added { edge });
    }

    fn decision_added(&self, d: &Decision) {
        let decision = orch::shown_decision(d.clone());
        self.send(&Audience::Card(&d.card_id), &Push::decision_added { decision });
    }

    fn landing_updated(&self, l: &wire::app::LandingRead) {
        let landing = l.clone();
        self.send(&Audience::Owner, &Push::landing_updated { landing });
    }

    fn event_added(&self, e: &Event) {
        let event = orch::event_read(e.clone());
        self.send(&Audience::Session(&e.card_id), &Push::event_added { event });
    }

    fn event_updated(&self, e: &Event) {
        let event = orch::event_read(e.clone());
        self.send(&Audience::Session(&e.card_id), &Push::event_updated { event });
    }

    fn inbox_updated(&self, it: &InboxItem) {
        let item = it.clone();
        self.send(&Audience::Owner, &Push::inbox_updated { item });
    }

    fn employee_updated(&self, e: &Employee) {
        self.send(
            &Audience::Owner,
            &Push::employee_updated {
                employee: orch::employee_read_of(e),
            },
        );
    }

    fn client_key_updated(&self, k: &ClientKey) {
        self.send(&Audience::Owner, &Push::client_key_updated { key: k.clone() });
    }

    fn client_key_deleted(&self, id: &str) {
        let gone = ClientKeyGone { id: id.to_owned() };
        self.send(&Audience::Owner, &Push::client_key_deleted { gone });
    }

    /// Closes the sessions signed in with the key: its mentor may come in no more.
    fn worker_key_deleted(&self, id: &str) {
        self.subs().by_id.retain(|_, s| match &s.seat {
            Seat::Mentor(m) if m.key.id == id => {
                s.behind.cancel();
                false
            }
            Seat::Owner | Seat::Mentor(_) => true,
        });
    }

    fn draft_updated(&self, d: &Draft) {
        let draft = d.clone();
        self.send(&Audience::Writer(&d.mentor), &Push::draft_updated { draft });
    }

    fn draft_deleted(&self, d: &Draft) {
        let gone = DraftGone {
            id: d.id.clone(),
            home: d.home.clone(),
        };
        self.send(&Audience::Writer(&d.mentor), &Push::draft_deleted { gone });
    }

    fn health_updated(&self, h: &wire::app::Health) {
        self.send(&Audience::Owner, &Push::health_updated { health: h.clone() });
    }

    fn card_shown(&self, s: &wire::app::CardShown) {
        self.send(&Audience::Card(&s.id), &Push::card_shown { shown: s.clone() });
    }

    fn event_delta(&self, d: &wire::app::EventDelta) {
        self.send(&Audience::Session(&d.card_id), &Push::event_delta { delta: d.clone() });
    }

    fn event_deleted(&self, e: &Event) {
        let gone = EventGone {
            id: e.id,
            home: e.home.clone(),
            card_id: e.card_id.clone(),
        };
        self.send(&Audience::Session(&e.card_id), &Push::event_deleted { gone });
    }

    fn mentor_updated(&self, m: &model::Mentor) {
        self.send(&Audience::Owner, &Push::mentor_updated { mentor: m.clone() });
    }

    fn workload_updated(&self, w: &wire::app::Workload) {
        self.send(&Audience::Owner, &Push::workload_updated { workload: w.clone() });
    }

    /// The owner's session and the help's mentor's are pushed it; the mentor's, their board laid out again with it.
    fn help_updated(&self, h: &model::Help) {
        let push = Push::help_updated { help: h.clone() };
        self.subs().by_id.retain(|_, s| {
            let board = match &mut s.seat {
                Seat::Owner => return queue(s, push.clone()),
                Seat::Mentor(m) => {
                    if !m.learn(h) {
                        return true;
                    }
                    Some(orch::mentor_board(&m.helps))
                }
            };
            queue(s, push.clone()) && board.is_none_or(|board| queue(s, Push::board_updated { board }))
        });
    }

    fn workflow_updated(&self, run: &model::Workflow) {
        self.send(&Audience::Owner, &Push::workflow_updated { workflow: run.clone() });
    }

    fn boards_updated(&self, b: &wire::app::Boards) {
        self.send(&Audience::Owner, &Push::boards_updated { boards: b.clone() });
    }

    fn at_work_updated(&self, v: &wire::app::AtWork) {
        self.send(&Audience::Owner, &Push::at_work_updated { at_work: v.clone() });
    }

    fn event_signal_updated(&self, es: &model::EventSignal) {
        self.send(
            &Audience::Owner,
            &Push::event_signal_updated {
                event_signal: es.clone(),
            },
        );
    }

    fn event_signal_deleted(&self, id: &str) {
        let gone = EventSignalGone { id: id.to_owned() };
        self.send(&Audience::Owner, &Push::event_signal_deleted { gone });
    }

    fn sensor_updated(&self, s: &model::Sensor) {
        self.send(&Audience::Owner, &Push::sensor_updated { sensor: s.clone() });
    }

    fn sensor_deleted(&self, id: &str) {
        let gone = SensorGone { id: id.to_owned() };
        self.send(&Audience::Owner, &Push::sensor_deleted { gone });
    }

    fn source_updated(&self, s: &wire::app::ToolSource) {
        self.send(&Audience::Owner, &Push::source_updated { source: s.clone() });
    }

    fn extension_updated(&self, e: &model::Extension) {
        self.send(&Audience::Owner, &Push::extension_updated { extension: e.clone() });
    }

    fn extension_deleted(&self, id: &str) {
        let gone = ExtensionGone { id: id.to_owned() };
        self.send(&Audience::Owner, &Push::extension_deleted { gone });
    }

    fn playbook_updated(&self, p: &model::Playbook) {
        self.send(&Audience::Owner, &Push::playbook_updated { playbook: p.clone() });
    }

    fn playbook_deleted(&self, id: &str) {
        let gone = PlaybookGone { id: id.to_owned() };
        self.send(&Audience::Owner, &Push::playbook_deleted { gone });
    }

    fn agent_updated(&self, a: &model::Agent) {
        self.send(&Audience::Owner, &Push::agent_updated { agent: a.clone() });
    }

    fn agent_deleted(&self, id: &str) {
        let gone = AgentGone { id: id.to_owned() };
        self.send(&Audience::Owner, &Push::agent_deleted { gone });
    }

    fn skill_deleted(&self, name: &str) {
        let gone = SkillGone { name: name.to_owned() };
        self.send(&Audience::Owner, &Push::skill_deleted { gone });
    }

    fn feedback_updated(&self, f: &model::Feedback) {
        self.send(&Audience::Owner, &Push::feedback_updated { feedback: f.clone() });
    }

    fn insight_added(&self, d: &Decision) {
        let insight = d.clone();
        self.send(&Audience::Owner, &Push::insight_added { insight });
    }

    fn objective_updated(&self, v: &model::ObjectiveView) {
        self.send(&Audience::Owner, &Push::objective_updated { view: v.clone() });
    }

    fn project_updated(&self, p: &model::Project) {
        self.send(&Audience::Owner, &Push::project_updated { project: p.clone() });
    }
}

impl orch::pipeline::notify::Notifier for Broadcast {
    fn event_read(&self, e: &wire::app::EventRecord) {
        self.send(&Audience::Owner, &Push::event_read { event: e.clone() });
    }

    fn signal_board_updated(&self, b: &wire::app::SignalBoard) {
        self.send(&Audience::Owner, &Push::signal_board_updated { board: b.clone() });
    }
}

impl orch::capability::notify::Notifier for Broadcast {
    fn skill_updated(&self, s: &wire::app::ListedSkill) {
        self.send(&Audience::Owner, &Push::skill_updated { skill: s.clone() });
    }
}

impl orch::crew::notify::Notifier for Broadcast {
    fn retrospective_added(&self, r: &wire::app::RetrospectiveRead) {
        self.send(
            &Audience::Owner,
            &Push::retrospective_added {
                retrospective: r.clone(),
            },
        );
    }
}

impl orch::Notifier for Broadcast {
    fn assistant_feed_updated(&self, f: &wire::app::AssistantFeed) {
        self.send(&Audience::Owner, &Push::assistant_feed_updated { feed: f.clone() });
    }

    fn mentor_standing_updated(&self, s: &wire::app::MentorStanding) {
        self.send(&Audience::Owner, &Push::mentor_standing_updated { mentor: s.clone() });
    }

    fn board_updated(&self, b: &wire::app::Board) {
        self.send(&Audience::Owner, &Push::board_updated { board: b.clone() });
    }

    fn chain_standing_updated(&self, s: &wire::app::ChainStanding) {
        let push = Push::chain_standing_updated { standing: s.clone() };
        self.send_with(&Audience::Chain(&s.chain_id), &push, |helps| {
            let standing = orch::mentor_chain_standing(helps, s.clone())?;
            Some(Push::chain_standing_updated { standing })
        });
    }

    fn repo_updated(&self, v: &wire::app::RepoView) {
        let view = v.clone();
        self.send(&Audience::Owner, &Push::repo_updated { view });
    }

    fn verification_updated(&self, v: &wire::app::Verification) {
        self.send(
            &Audience::Owner,
            &Push::verification_updated {
                verification: v.clone(),
            },
        );
    }

    fn agent_standing_updated(&self, a: &wire::app::AgentStanding) {
        let push = Push::agent_standing_updated { agent: a.clone() };
        self.send_with(&Audience::Card(&a.card_id), &push, |helps| {
            let agent = orch::mentor_agent_standing(a.clone(), helps)?;
            Some(Push::agent_standing_updated { agent })
        });
    }
}
