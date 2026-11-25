//! A run is one session of a card: its agent process, from the moment it is opened until it is closed. The orchestrator
//! holds the run of each card that has one (`Shared::runs`); a run's own state sits behind its lock, taken briefly and
//! never across an await. What the session's process reports reaches the run through its `RunHandler`, in the order it
//! happened: the handler queues each report and one task per process handles them one at a time. A child card's run
//! (spawn.rs) works in its root's process: its handler queues onto the root's, so what records the child comes before
//! anything its subagent does.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::Duration;

use model::{
    Actor, ActorKind, Card, CardRole, CardStatus, Cut, Draft, Event, EventKind, EventStatus, JudgeKind, Retry,
    SystemNote, Time,
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;
use wire::app::Delivery;

use crate::apply::Records;
use crate::background::OwnTurn;
use crate::chain_core::Core;
use crate::chain_core::new_id;
use crate::health::elapsed;
use crate::images::{images_in, with_images};
use crate::inbox::{Cause, Failed};
use crate::mailbox::Mailbox;
use crate::outbox::{Message, owner_message, restart_by, sent_by};
use crate::permission::{AskedPermission, unanswered};
use crate::recovery::{LOST_CONTEXT_MARKER, recovery_of};
use crate::request::Service;
use crate::spawn::SpawnStep;
use crate::turn_end::{TurnEnd, WorkerReport};
use crate::{Error, Request, Result};

/// One session of a card.
pub struct Run {
    /// Names the agent process in the usage ledger.
    pub id: String,
    /// The card's home (its chain, or the Assistant), which never changes.
    pub home: String,
    /// The session to resume instead of starting a new one.
    pub resume: String,
    /// The MCP bearer token of the session's agent.
    pub token: String,
    /// Open while the session is: closing it, under the run's lock, is what ends the run.
    pub mailbox: Mailbox,
    /// Held while a queued message is handed to the session, as a turn's message (take) or into its turn (steer).
    pub take_mu: tokio::sync::Mutex<()>,
    /// The run whose process this one's agent runs in, for a child card (spawn.rs); none for a run with a session of
    /// its own. Set before the run is shared, never changed.
    pub root: Option<Arc<Run>>,
    state: Mutex<RunState>,
}

/// What a run holds that changes.
#[derive(Default)]
pub struct RunState {
    pub card: Card,
    pub sess: Option<Arc<dyn agent::Session>>,
    /// Agent message text of the current turn.
    pub turn: String,
    /// Why the current turn was cut short (`cancel_turn`); none while it was not.
    pub cut: Option<Cut>,
    /// The message or thought event chunks are being appended to.
    pub stream: Option<Event>,
    /// The stream's text length in UTF-16 code units: the offset of the next chunk (stream.rs's `EventDelta`).
    pub stream_len: usize,
    /// Last database write of the stream.
    pub saved_at: Time,
    /// Last push of what the card shows of the moment (stream.rs's `show`).
    pub pushed_at: Time,
    /// A push of it is on its way.
    pub pushing: bool,
    /// The permission prompts the session waits on, by request id (permission.rs).
    pub permissions: HashMap<String, AskedPermission>,
    /// The message whose turn is open, from its take until its prompt is answered; 0 when none.
    pub prompt: i64,
    /// The turn of its own the agent is in (background.rs's `on_working`).
    pub own: Option<Arc<OwnTurn>>,
    /// Turns of its own the agent took that the loop has not recorded yet, oldest first.
    pub own_turns: VecDeque<Arc<OwnTurn>>,
    /// The event of each background task of the agent's process, by the process's task id.
    pub tasks: HashMap<String, i64>,
    /// How background tasks ended since the agent last went to work: what opens a turn of its own.
    pub news: Vec<String>,
    /// When the agent was last heard from (anything its process sent) or its turn began.
    pub heard: Time,
    /// When the turn in progress was cut short because the agent had sent nothing for too long; none otherwise.
    pub silenced: Option<Time>,
    /// What the repetition rail knows of the turn in progress (repeats.rs).
    pub repeats: crate::repeats::Repeats,
    /// Who opened the turn in progress: the sender of its message, or the agent itself for a turn of its own.
    pub opener: Actor,
    /// The agent wrote its plan in the turn in progress (update_plan).
    pub plan_written: bool,
    /// The agent reported to its Chair in the turn in progress (the report tool).
    pub reported: bool,
    /// Opens the session's first message; recovery keeps it until the agent finishes a turn that received it.
    pub lead: String,
    /// Replaces an earlier lead of the same kind on a message sent again.
    pub lead_marker: String,
    /// The pending recovery of the session's lost context, when there is one.
    pub recovery: Option<Event>,
    /// The session is new and its first message is not sent yet: the next turn's message is recorded as its opening.
    pub opening_pending: bool,
    /// The memories the session was given, with its turns' messages and at moments of its work (recall.rs, ADR-0117): it
    /// is given others later, until a compaction leaves it a summary of them.
    pub recalled: HashSet<String>,
    /// The child runs of the subagents this run's process runs, its nested spawns among them, by task id (spawn.rs);
    /// only a run with a session of its own has them.
    pub children: HashMap<String, Arc<Run>>,
}

impl Run {
    /// A new session of card c: its own session resumed when it had one.
    pub fn new(c: Card) -> Self {
        Self::of(c, None)
    }

    /// The run of child card c, at work in root's process (spawn.rs): it holds no session of its own.
    pub fn child_of(c: Card, root: Arc<Run>) -> Self {
        Self::of(c, Some(root))
    }

    fn of(c: Card, root: Option<Arc<Run>>) -> Self {
        Self {
            id: new_id("r_"),
            home: c.home.clone(),
            resume: c.session_id.clone(),
            token: new_id("t_"),
            mailbox: Mailbox::default(),
            take_mu: tokio::sync::Mutex::new(()),
            root,
            state: Mutex::new(RunState {
                card: c,
                ..RunState::default()
            }),
        }
    }

    pub fn state(&self) -> MutexGuard<'_, RunState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn card_id(&self) -> String {
        self.state().card.id.clone()
    }

    /// A copy of the card as the run holds it now.
    pub fn card_now(&self) -> Card {
        self.state().card.clone()
    }

    pub fn created_at(&self) -> Time {
        self.state().card.created_at
    }

    /// Whether the run's agent is in a turn.
    pub fn busy(&self) -> bool {
        let s = self.state();
        s.sess.is_some() && matches!(s.card.status(), CardStatus::Running | CardStatus::Permission)
    }

    /// Notes that the agent sent something at `at`.
    pub fn hear(&self, at: Time) {
        self.state().heard = at;
    }

    /// Whether the session is still open.
    pub fn open(&self) -> bool {
        !self.mailbox.is_closed()
    }

    /// Closes the mailbox under the run's lock, so no take is half done; false when it was closed already.
    pub fn close_box(&self) -> bool {
        let _s = self.state();
        self.mailbox.close()
    }

    /// Refuses the permission requests r's turn waits on, the turn cut short for why, and returns r's session (none when
    /// it has none).
    pub fn end_asked(&self, why: &Cut) -> Option<Arc<dyn agent::Session>> {
        let mut s = self.state();
        // dropping each request's answer sender wakes its waiter, which reads it as unanswered
        s.permissions.clear();
        s.cut = Some(why.clone());
        s.sess.clone()
    }

    /// Ends the turn r's agent is in, cut short for why. The permission requests the turn waits on are answered first,
    /// refused: the agent would otherwise go on waiting for the owner.
    pub async fn cancel_turn(&self, why: &Cut) -> Result<()> {
        let sess = self
            .end_asked(why)
            .ok_or_else(|| Error::refused("session not started yet"))?;
        sess.cancel().await.map_err(|err| Error::refused(err.to_string()))
    }

    /// Closes r as Shutdown does, for a turn or a process the server's stop ended before Shutdown reached r: its end is
    /// not recorded, its messages stay queued, and the tool calls its turn left open say the server stopped.
    pub fn close_for_stop(&self) {
        let mut s = self.state();
        self.mailbox.close();
        s.cut = Some(Cut::ServerStopped);
    }
}

impl RunState {
    /// Notes that a turn begins at `at`: its silence counts from then, and its repeats from nothing.
    pub fn begin_turn(&mut self, at: Time, opener: Actor) {
        self.heard = at;
        self.silenced = None;
        self.repeats = crate::repeats::Repeats::default();
        self.opener = opener;
        self.plan_written = false;
        self.reported = false;
    }
}

/// What a session's process reported, queued for the process's task in the order it happened.
enum Report {
    Update(agent::Update),
    Init(model::SessionInit),
    Context(Box<model::SessionContext>),
    Task(wire::runtime::Task),
    Working(bool),
    Exit(Option<agent::Exit>),
    /// A subagent's start, refusal or end, as the process said it (spawn.rs).
    Spawn(Box<SpawnStep>),
    /// A child card's permission request: asked in its place in the queue, after the report that records the card.
    Permission(
        wire::runtime::PermissionRequest,
        oneshot::Sender<wire::runtime::PermissionAnswer>,
    ),
}

/// A report and the run it is about: a process's own, or a child card's in it.
type Reports = mpsc::UnboundedSender<(Arc<Run>, Report)>;

/// The `agent::Handler` of one run: what its process sends is about that run, or about a child card's run in it, whose
/// handler is one of these too (`on_subagent`). Once the card has another run (reopened) or none (forgotten with its
/// chain), the process belongs to the past and what it sends is ignored: its updates are dropped, its permission
/// requests refused, its exit is not the card's. Whatever the current run's process sends also says its agent is alive:
/// the liveness rail counts from it. It holds its run weakly: the run holds the session, which holds its handler, so a
/// strong hold would keep every run and session ever opened alive.
struct RunHandler {
    o: Weak<Core>,
    r: Weak<Run>,
    reports: Reports,
}

impl RunHandler {
    /// The orchestrator and the run, while the run's process is its card's run.
    fn current(&self) -> Option<(Arc<Core>, Arc<Run>)> {
        let (o, r) = (self.o.upgrade()?, self.r.upgrade()?);
        o.current(&r.process()).then_some((o, r))
    }

    fn report(&self, rep: Report) {
        if let Some((o, r)) = self.current() {
            self.queue(&o, &r, Arc::clone(&r), rep);
        }
    }

    /// Queues rep about run `about` (r, r's process or a child card's run in it), r being current.
    fn queue(&self, o: &Core, r: &Arc<Run>, about: Arc<Run>, rep: Report) {
        r.process().hear(o.now());
        // a closed queue: the process's task has ended with the orchestrator's work, and nothing more is recorded
        self.reports.send((about, rep)).ok();
    }
}

impl agent::Handler for RunHandler {
    fn on_update(&self, u: agent::Update) {
        self.report(Report::Update(u));
    }

    fn on_permission(
        &self,
        req: wire::runtime::PermissionRequest,
    ) -> agent::BoxFuture<'_, wire::runtime::PermissionAnswer> {
        Box::pin(async move {
            let Some((o, r)) = self.current() else {
                return unanswered();
            };
            if r.root.is_some() {
                // a child's card may not be recorded yet: its request waits behind the report that records it
                let (tx, rx) = oneshot::channel();
                self.queue(&o, &r, Arc::clone(&r), Report::Permission(req, tx));
                return rx.await.unwrap_or_else(|_| unanswered());
            }
            r.hear(o.now());
            let d = o.on_permission(&r, req).await;
            // the wait for the answer is the client's, not the agent's silence: its clock starts again once it has it
            r.hear(o.now());
            d
        })
    }

    fn on_init(&self, init: model::SessionInit) {
        self.report(Report::Init(init));
    }

    fn on_context(&self, context: model::SessionContext) {
        self.report(Report::Context(Box::new(context)));
    }

    fn on_task(&self, t: wire::runtime::Task) {
        if t.kind != wire::runtime::TASK_SUBAGENT {
            self.report(Report::Task(t));
            return;
        }
        // worked out here, as the process says it, so the child's run exists before its subagent's first word asks for
        // its handler (on_subagent); recorded in the queue's order
        if let Some((o, r)) = self.current()
            && let Some(step) = o.spawn_task(&r, t)
        {
            self.queue(&o, &r, r.process(), Report::Spawn(Box::new(step)));
        }
    }

    fn on_working(&self, working: bool) {
        self.report(Report::Working(working));
    }

    /// What the provider says of the limit of the account the card runs on: the pool and the owner read it at once.
    fn on_limit(&self, l: wire::runtime::Limit) {
        if let Some((o, r)) = self.current() {
            let account = r.process().card_now().account;
            if !account.is_empty() {
                o.account_limit(&account, l);
            }
        }
    }

    /// Queued even for a run that is no longer its card's: the exit is the process's last report, which ends its task.
    /// A run already dropped has no task waiting: its queue closed with its session.
    fn on_exit(&self, end: Option<agent::Exit>) {
        if let Some(r) = self.r.upgrade() {
            self.reports.send((r, Report::Exit(end))).ok();
        }
    }

    /// What the agent must do before its turn ends (turn_end.rs's `owed_before_end`); nothing for a process that is no
    /// longer its card's.
    fn on_stop(&self) -> agent::BoxFuture<'_, Option<String>> {
        Box::pin(async move {
            let (o, r) = self.current()?;
            o.owed_before_end(&r)
        })
    }

    /// What the agent reads beside a moment of its work, of what its card remembers (recall.rs's `told_at`); nothing for
    /// a process that is no longer its card's.
    fn on_moment(&self, m: wire::runtime::Moment) -> agent::BoxFuture<'_, Option<String>> {
        Box::pin(async move {
            let (o, r) = self.current()?;
            o.told_at(&r, m).await
        })
    }

    /// The handler of the child card subagent agent_id of this run's process works in (spawn.rs): what it does lands on
    /// that card. None for a spawn that has no card (refused) or whose card has ended.
    fn on_subagent(&self, agent_id: &str) -> Option<Arc<dyn agent::Handler>> {
        let (_, r) = self.current()?;
        let child = r.child(agent_id)?;
        Some(Arc::new(RunHandler {
            o: Weak::clone(&self.o),
            r: Arc::downgrade(&child),
            reports: self.reports.clone(),
        }))
    }
}

/// How long a session may take to start.
const START_WAIT: Duration = Duration::from_secs(3 * 60);
/// How long a cancel request may take to reach an agent.
pub const CANCEL_WAIT: Duration = Duration::from_secs(5);

impl Core {
    /// The run of card, when it has one.
    pub fn run_of(&self, card: &str) -> Option<Arc<Run>> {
        self.shared().runs.get(card).cloned()
    }

    /// Whether r is its card's run.
    pub fn current(&self, r: &Arc<Run>) -> bool {
        self.run_of(&r.card_id()).is_some_and(|now| Arc::ptr_eq(&now, r))
    }

    /// The runs of every card.
    pub fn all_runs(&self) -> Vec<Arc<Run>> {
        self.shared().runs.values().cloned().collect()
    }

    /// The runs of the cards of a chain.
    pub fn chain_runs(&self, chain: &str) -> Vec<Arc<Run>> {
        self.shared()
            .runs
            .values()
            .filter(|r| r.home == chain)
            .cloned()
            .collect()
    }

    /// Whether card has a session open now.
    pub fn session_open(&self, card: &str) -> bool {
        self.run_of(card).is_some_and(|r| r.open())
    }

    /// Runs a card's session loop as work Shutdown waits for. Once Shutdown has begun no loop starts: the card keeps its
    /// queued messages and the next start delivers them.
    pub fn go_loop(self: &Arc<Self>, r: Arc<Run>) {
        let o = Arc::clone(self);
        self.background(move |work| async move { o.session_loop(&work, &r).await });
    }

    /// Starts the run's session and works through the card's queued messages one turn at a time, while the chain is not
    /// paused, until the run closes. A turn the agent takes on its own comes first: it runs already.
    async fn session_loop(self: &Arc<Self>, work: &CancellationToken, r: &Arc<Run>) {
        // a card whose provider is out of use opens no session there to find out
        if self.is_stopping() || self.unreachable(r, None).await {
            return;
        }
        let Some(c) = self.start_session(r).await else { return };
        loop {
            if work.is_cancelled() {
                return;
            }
            let own = r.state().own_turns.pop_front();
            if let Some(t) = own {
                let span = turn_span(r, t.opening_id());
                // boxed, as the turn below: a turn's state stays off this long-lived loop's future and stack frame
                Box::pin(self.run_own_turn(r, &t).instrument(span)).await;
                continue;
            }
            if self.wait_open(&c).await.is_err() {
                return;
            }
            if let Some(e) = self.take(r).await {
                // checked on the message taken, so none slips in after the check
                if self.unreachable(r, Some(&e)).await {
                    return;
                }
                let Some(e) = self.with_recall(work, r, e).await else {
                    return; // stopped or shutting down while it recalled: the message is the recovery's to resume
                };
                let opening = {
                    let mut s = r.state();
                    let pending = std::mem::take(&mut s.opening_pending);
                    s.sess
                        .as_ref()
                        .filter(|_| pending)
                        .map(|sess| (s.card.id.clone(), sess.id().to_owned()))
                };
                if let Some((card, session)) = opening {
                    // the text the turn sends (failover's prompt_retrying)
                    self.record_opening(&card, &session, crate::outbox::read_of(&e)).await;
                }
                let span = turn_span(r, e.id);
                // boxed: a turn runs everything that follows it (its end, the report, the next change of the chain), and
                // inline it would make this loop's future and debug-build stack frame as deep as all of that
                Box::pin(self.turn(r, e).instrument(span)).await;
                continue;
            }
            tokio::select! {
                open = r.mailbox.wait() => if !open { return },
                () = work.cancelled() => return,
            }
        }
    }

    /// Opens run r's session (its own resumed, or a new one reading first what a lost context or a pending recovery
    /// leaves) and returns the card as the run began; none when it could not, its failure already handled.
    async fn start_session(self: &Arc<Self>, r: &Arc<Run>) -> Option<Card> {
        let (c, sess) = self.launch(r).await?;
        let closed = {
            let mut s = r.state(); // under the run's lock, as close_run_obj closes it: end_run finds this session or never will
            let closed = r.mailbox.is_closed();
            if !closed {
                s.sess = Some(Arc::clone(&sess));
            }
            closed
        };
        if closed {
            sess.close(); // stopped while its session started
            return None;
        }
        if sess.context_lost() {
            let brief = match self.recovery_brief(&c).await {
                Ok(b) => b,
                Err(err) => {
                    self.fail_to_open(r, &c, &format!("restore the session's brief: {err}"))
                        .await;
                    return None;
                }
            };
            let mut s = r.state();
            LOST_CONTEXT_MARKER.clone_into(&mut s.lead_marker);
            s.lead = format!(
                "{LOST_CONTEXT_MARKER} because the session could not be resumed. Check git status and git log in your \
                 worktree before continuing.]\n\n{brief}"
            );
        } else if !r.resume.is_empty() {
            let (card, id) = (c.id.clone(), sess.id().to_owned());
            match self.store.read(move |s| s.pending_session_recovery(&card, &id)).await {
                Err(store::Error::NotFound) => {}
                Err(err) => {
                    self.fail_to_open(r, &c, &format!("read pending session recovery: {err}"))
                        .await;
                    return None;
                }
                Ok(recovery) => match recovery_of(&recovery) {
                    Err(err) => {
                        self.fail_to_open(r, &c, &err.to_string()).await;
                        return None;
                    }
                    Ok(pending) => {
                        let mut s = r.state();
                        s.lead = pending.lead;
                        LOST_CONTEXT_MARKER.clone_into(&mut s.lead_marker);
                        s.recovery = Some(recovery);
                    }
                },
            }
        }
        if !sess.context_lost() && !r.resume.is_empty() {
            self.seed_recalled(r).await; // the session it resumes holds what it was given
        }
        // A replacement's id is saved with its brief when take stores the first prompt. Until then another restart must
        // recover the missing session again, rather than resume a new session that never received its brief.
        let id = (!sess.context_lost()).then(|| sess.id().to_owned());
        let now = self.now();
        let stored = self
            .update_card(&c.id, move |c| {
                if let Some(id) = &id {
                    c.session_id.clone_from(id);
                }
                c.set_status(CardStatus::Idle, now);
            })
            .await;
        if let Err(err) = stored {
            // a session whose id is not stored could not be resumed after it closes: it takes no message
            self.fail_to_open(r, &c, &format!("record the session: {err}")).await;
            return None;
        }
        Some(c)
    }

    /// Starts the session of run r's card and returns the card as it starts; none when it could not, its failure already
    /// handled. The session runs Claude Code on the account the card is placed on, and goes on another when that one fails
    /// it.
    async fn launch(self: &Arc<Self>, r: &Arc<Run>) -> Option<(Card, Arc<dyn agent::Session>)> {
        let c = match self.bind_account(r.card_now()).await {
            Ok(c) => c,
            Err(err) => {
                let c = r.card_now();
                self.fail_to_open(r, &c, &format!("choose the account: {err}")).await;
                return None;
            }
        };
        let (tx, rx) = mpsc::unbounded_channel();
        let h = Arc::new(RunHandler {
            o: Arc::downgrade(self),
            r: Arc::downgrade(r),
            reports: tx,
        });
        let started = tokio::time::timeout(START_WAIT, async {
            let (spec, acc) = self
                .domains()
                .map_err(|e| (e.to_string(), None))?
                .spec(&c, &r.resume, &r.token)
                .await
                .map_err(|e| (e.to_string(), None))?;
            let sent = spec.clone();
            let sess = self.open_session(spec, h).await.map_err(|e| {
                let f = crate::failover::provider_fault(&e);
                (e.to_string(), f)
            })?;
            Ok((sess, sent, acc))
        })
        .await
        .unwrap_or_else(|_| Err(("it did not start within 3 minutes".to_owned(), None)));
        match started {
            Ok((sess, sent, acc)) => {
                let sess: Arc<dyn agent::Session> = Arc::from(sess);
                let resumed = !r.resume.is_empty() && !sess.context_lost();
                // what the owner reads of the start; the session runs without it
                let launched = match self.domains() {
                    Ok(d) => d.record_launch(&c, &sent, &acc, sess.id(), resumed).await,
                    Err(err) => Err(err),
                };
                if let Err(err) = launched {
                    tracing::error!(home = %c.home, card = %c.id, %err, "record the session's start");
                }
                if !resumed {
                    r.state().opening_pending = true;
                }
                self.pump(rx);
                Some((c, sess))
            }
            Err((why, f)) => {
                if let Some(f) = f
                    && self.failover(r, &f, None, &[]).await
                {
                    return None;
                }
                // its messages stay queued: the owner's retry opens the session again and it reads them
                self.fail_to_open(r, &c, &format!("start session: {why}")).await;
                None
            }
        }
    }

    /// Handles what the run's session reports, of it and of the child cards in its process, one report at a time, until
    /// its process has ended (its exit is the last report), its session is gone (its handler holds the queue) or the work
    /// is cut short.
    fn pump(self: &Arc<Self>, mut reports: mpsc::UnboundedReceiver<(Arc<Run>, Report)>) {
        let o = Arc::clone(self);
        self.background(move |work| async move {
            loop {
                let (r, rep) = tokio::select! {
                    biased;
                    next = reports.recv() => match next {
                        Some(next) => next,
                        None => return,
                    },
                    () = work.cancelled() => return,
                };
                let last = matches!(rep, Report::Exit(_));
                if o.current(&r) {
                    o.on_report(&r, rep).await;
                }
                if last {
                    return;
                }
            }
        });
    }

    /// Handles one report of r's process.
    async fn on_report(self: &Arc<Self>, r: &Arc<Run>, rep: Report) {
        match rep {
            Report::Update(u) => self.on_update(r, u).await,
            Report::Init(init) => self.on_init(r, init).await,
            Report::Context(c) => self.on_context(r, *c).await,
            Report::Task(t) => {
                if t.kind == wire::runtime::TASK_WORKFLOW {
                    // a run of Claude Code's Workflow: its record (engine claude), and its task in the card's stream
                    self.on_workflow_task(r, &t).await;
                }
                self.on_task(r, t).await;
            }
            Report::Working(w) => self.on_working(r, w).await,
            Report::Exit(end) => self.on_exit(r, end).await,
            Report::Spawn(step) => self.on_spawn_step(*step).await,
            Report::Permission(req, tx) => self.ask_for_child(Arc::clone(r), req, tx),
        }
    }

    /// Asks child card r's permission request on its own, beside the queue, which goes on meanwhile: the answer may be
    /// the owner's, long after. The subagent gives up waiting when it drops tx's other end.
    fn ask_for_child(
        self: &Arc<Self>,
        r: Arc<Run>,
        req: wire::runtime::PermissionRequest,
        mut tx: oneshot::Sender<wire::runtime::PermissionAnswer>,
    ) {
        let o = Arc::clone(self);
        self.background(move |_| async move {
            let decided = tokio::select! {
                d = o.on_permission(&r, req) => Some(d),
                () = tx.closed() => None,
            };
            if let Some(d) = decided {
                tx.send(d).ok(); // the subagent stopped waiting
            }
        });
    }

    /// The process of r's session ended.
    async fn on_exit(self: &Arc<Self>, r: &Arc<Run>, end: Option<agent::Exit>) {
        if self.stop_begun() {
            r.close_for_stop(); // the stop ended it: the next start resumes the card
            return;
        }
        match end {
            Some(agent::Exit::HostRestarted) => self.host_restarted(r).await, // the runtime died under it: it resumes
            Some(end) => {
                self.fail(r, &Failed::new(Cause::ProcessEnded, end.to_string())).await;
            }
            None => {
                self.fail(r, &Failed::new(Cause::ProcessEnded, "agent process exited"))
                    .await;
            }
        }
    }

    /// Runs the agent on message e and records its end. A turn that a server restart cut off and its continuation
    /// resume answers the message that turn was on.
    async fn turn(self: &Arc<Self>, r: &Arc<Run>, e: Event) {
        self.close_stream(r).await;
        let (sess, cut_before) = {
            let mut s = r.state();
            s.turn.clear();
            (s.sess.clone(), s.cut.clone())
        };
        let Some(sess) = sess else { return };
        if cut_before == Some(Cut::Paused) {
            // paused before the agent read it: it waits for the resume in its queue, as an untaken message does
            self.put_back(r, &e).await;
            return;
        }
        let started = self.now();
        self.saw_work(&r.home, started);
        // a turn cut short before its message was sent ends as one cut at its first moment, the agent never reading it
        let done = if cut_before.is_some() {
            Ok(model::StopReason::Cancelled)
        } else {
            self.prompt_retrying(r, sess.as_ref(), &e).await
        };
        let host_died = matches!(
            &done,
            Err(agent::Error::HostRestarted) | Err(agent::Error::Ended(Some(agent::Exit::HostRestarted)))
        );
        let (silenced, cut) = {
            let mut s = r.state();
            if !host_died {
                // a turn the runtime's end cut stays open: host_restarted reads it as cut mid-turn
                s.prompt = 0;
            }
            s.news.clear(); // what background work told the agent until now, it was told in this turn
            if host_died && s.cut.is_none() {
                s.cut = Some(Cut::RuntimeRestarted);
            }
            (s.silenced.is_some(), s.cut.clone())
        };
        let done: std::result::Result<model::StopReason, (Failed, Option<wire::runtime::Failure>)> = match done {
            Ok(stop) if ended_by_silence(silenced, stop) => Err((Failed::new(Cause::Silent, self.silence()), None)),
            Ok(stop) => Ok(stop),
            Err(err) => {
                let f = crate::failover::provider_fault(&err);
                Err((Failed::of_turn(&err), f))
            }
        };
        if done.is_err() && self.stop_begun() {
            r.close_for_stop(); // the stop failed it: the next start resumes the turn
        }
        self.end_open_calls(r, &e, done.as_ref().err().map(|(why, _)| why.text.as_str()))
            .await;
        if r.mailbox.is_closed() {
            return; // closed under it: its end is not recorded
        }
        if host_died {
            self.host_restarted(r).await;
            return;
        }
        let stop = match done {
            Err((why, f)) => {
                self.end_failed_turn(r, &e, &why, f.as_ref()).await;
                return;
            }
            Ok(stop) => stop,
        };
        self.account_answered(r);
        self.supersede_errors(&e.card_id).await;
        self.close_stream(r).await;
        let output = r.state().turn.trim().to_owned();
        // the turn's length and its row travel with its end, so any view (even one showing only part of the turn) can say
        // them
        let took = self.turn_data(&e, elapsed(started, self.now())).await;
        let stop_event = Event {
            home: e.home.clone(),
            card_id: e.card_id.clone(),
            status: EventStatus::Stop(stop),
            data: took,
            ..Event::default()
        };
        let answering = self.answering(&e).await;
        if let Err(err) = self.record_turn_stop(r, &e, stop_event).await {
            self.end_turn(
                r,
                &answering,
                TurnEnd::failed(
                    Failed::new(Cause::Unknown, format!("record turn completion: {err}")),
                    None,
                ),
            )
            .await;
            return;
        }
        if ended_by_pause(cut.as_ref(), stop) {
            self.hold_for_resume(r).await;
            return;
        }
        self.end_turn(
            r,
            &answering,
            TurnEnd {
                output,
                stop: stop.as_str().to_owned(),
                ..TurnEnd::default()
            },
        )
        .await;
    }

    /// The message a turn on message e answers: e, or, for the continuation of a turn a restart or a pause cut off, the
    /// message the work was on before its first cut (a turn may be cut more than once).
    pub async fn answering(&self, e: &Event) -> Event {
        let mut at = e.clone();
        while sent_by(&at) == restart_by() {
            let (card, id) = (at.card_id.clone(), at.id);
            match self.store.read(move |s| s.prompt_before(&card, id)).await {
                Ok(before) => at = before,
                Err(_) => break,
            }
        }
        at
    }

    /// Puts message e, taken for a turn of r that a pause cut before the agent read it, back in its card's queue, and the
    /// card at rest, in one change: once the chain runs again the card takes it as it was, with what it recalled.
    async fn put_back(self: &Arc<Self>, r: &Arc<Run>, e: &Event) {
        let card = r.card_now();
        let now = self.now();
        let w = Records {
            cards: vec![crate::step::CardEdit::new(&card.id, move |c: &mut Card| {
                c.set_status(CardStatus::Idle, now);
            })],
            ..Records::default()
        };
        {
            let mut s = r.state();
            s.prompt = 0;
            s.cut = None;
        }
        if let Err(err) = self.send(&card, crate::outbox::sent_again(e), w).await {
            tracing::error!(home = %card.home, card = %card.id, %err, "put back a message a pause cut before its turn");
        }
    }

    /// Holds r's turn that a pause cut until the chain resumes: the card rests, what it said reaches no one (it never
    /// finished the message it was on), and a message that goes on with that turn waits in its queue, in the same
    /// change. A paused chain's cards take nothing, so it is read once the chain runs again, and its turn answers the
    /// message the cut one was on (`answering`).
    async fn hold_for_resume(self: &Arc<Self>, r: &Arc<Run>) {
        let card = r.card_now();
        let m = Message {
            title: "Continue after the pause".to_owned(),
            text: PAUSE_CONTINUES.to_owned(),
            by: restart_by(),
            ..Message::default()
        };
        let now = self.now();
        let w = Records {
            cards: vec![crate::step::CardEdit::new(&card.id, move |c: &mut Card| {
                c.set_status(CardStatus::Idle, now);
            })],
            ..Records::default()
        };
        if let Err(err) = self.send(&card, m, w).await {
            tracing::error!(home = %card.home, card = %card.id, %err, "hold a turn the pause cut");
        }
    }

    /// Records that r's turn on message e failed with why and ends it: the turn moves to another model when its provider
    /// failed (failover), and is otherwise put in front of the owner, whose retry sends e again.
    pub async fn end_failed_turn(
        self: &Arc<Self>,
        r: &Arc<Run>,
        e: &Event,
        why: &Failed,
        f: Option<&wire::runtime::Failure>,
    ) {
        let failure = self.record_failure(r, &why.text).await;
        if let Some(f) = f
            && self.failover(r, f, Some(e), &failure).await
        {
            return;
        }
        let retry = Retry {
            card_id: e.card_id.clone(),
            prompt_id: e.id,
            failure_ids: failure,
            ..Retry::default()
        };
        let answering = self.answering(e).await;
        self.end_turn(r, &answering, TurnEnd::failed(why.clone(), Some(retry)))
            .await;
    }

    /// Ends the tool calls the turn on message e left open, saying why: what cut the turn short, or else how it ended.
    /// Nothing will end them now: the agent has given up on them.
    pub async fn end_open_calls(&self, r: &Run, e: &Event, failed: Option<&str>) {
        let cut = r.state().cut.clone();
        let why = match cut {
            Some(cut) => cut.to_string(),
            None if failed.is_some() => "The turn failed".to_owned(),
            None => "The turn ended".to_owned(),
        };
        if let Err(err) = self.end_tool_calls(&e.card_id, e.id, &why).await {
            tracing::error!(home = %e.home, card = %e.card_id, %err, "end the turn's tool calls");
        }
    }

    /// Ends a card's tool calls after event `after` that have not ended, because of why: they failed (a tool call ends
    /// completed or failed, and its result is lost), and say why.
    pub async fn end_tool_calls(&self, card_id: &str, after: i64, why: &str) -> Result<()> {
        let (card, now, text) = (card_id.to_owned(), self.now(), model::cut_short(why));
        let ended = self
            .store
            .update(move |tx| {
                let mut out = Vec::new();
                for mut e in tx.open_tool_calls(&card, after)? {
                    e.status = EventStatus::Tool(model::ToolStatus::Failed);
                    e.text.clone_from(&text);
                    e.updated_at = now;
                    out.push(tx.update_event(e)?);
                }
                Ok::<_, store::Error>(out)
            })
            .await?;
        for e in &ended {
            self.notify.event_updated(e);
        }
        Ok(())
    }

    /// Ends run r because its session broke: the card is failed and a worker's Chair hears of it. It returns the events
    /// that record the failure in the stream (`record_failure`). A run ends once: one that has already closed (stopped,
    /// put to sleep, failed, shut down) is not failed again, and fail says none.
    pub async fn fail(self: &Arc<Self>, r: &Arc<Run>, f: &Failed) -> Option<Vec<i64>> {
        // boxed: opening a session fails at many points, each awaiting this, and the report it sends would otherwise sit
        // inline in each of their futures and debug-build stack frames
        Box::pin(self.fail_run(r, f)).await
    }

    /// `fail`, unboxed.
    async fn fail_run(self: &Arc<Self>, r: &Arc<Run>, f: &Failed) -> Option<Vec<i64>> {
        if !self.close_run_obj(r, &Cut::SessionFailed) {
            return None;
        }
        Some(self.record_failed(r, f).await)
    }

    /// Records that run r, closed by its caller, failed with f, as `fail` does after closing it, and returns the events
    /// that record the failure.
    pub(crate) async fn record_failed(self: &Arc<Self>, r: &Arc<Run>, f: &Failed) -> Vec<i64> {
        let c = r.card_now();
        let why = f.text.as_str();
        tracing::error!(home = %c.home, card = %c.id, err = %why, "card failed");
        let failure = self.record_failure(r, why).await;
        let err = self.failure_title(&c, f);
        let now = self.now();
        let failed = move |c: &mut Card| {
            c.set_status(CardStatus::Failed, now);
            c.error.clone_from(&err);
        };
        match CardRole::of(&c) {
            CardRole::Judge(JudgeKind::Goal) => {
                // a failed check waits for no one: the chain finishes without it, and says so
                self.display(&c.id, failed).await;
                let why = format!("the judge failed: {why}");
                if let Err(err) = match self.domains() {
                    Ok(d) => {
                        d.end_goal_check(&c.home, &c.id, &why, false, crate::apply::Records::default())
                            .await
                    }
                    Err(err) => Err(err),
                } {
                    tracing::error!(home = %c.home, card = %c.id, %err, "goal check's end");
                }
                return failure;
            }
            CardRole::Judge(JudgeKind::Duel) => {
                // its duel ends without a pick, and the Chair judges both reports
                self.display(&c.id, failed).await;
                if let Ok(d) = self.domains() {
                    d.duel_judge_ended(&c, &format!("the judge failed: {why}")).await;
                }
                return failure;
            }
            CardRole::Judge(JudgeKind::Round) => {
                // its round ends without a pick, and every attempt's version goes back to its Chair
                self.display(&c.id, failed).await;
                if let Ok(d) = self.domains() {
                    d.round_judge_ended(&c, &format!("the judge failed: {why}")).await;
                }
                return failure;
            }
            CardRole::Judge(JudgeKind::Second) => {
                // its second opinion ends without a comparison; the Assistant's answer stands as it was
                self.display(&c.id, failed).await;
                if let Ok(d) = self.domains() {
                    d.second_judge_ended(&c, &format!("the judge failed: {why}")).await;
                }
                return failure;
            }
            CardRole::Chair | CardRole::Assistant => {
                // no one above it hears of it: the owner does
                self.display(&c.id, failed).await;
                return failure;
            }
            CardRole::Worker | CardRole::Spawned => {}
        }
        self.report(
            &c,
            WorkerReport {
                output: format!("Agent failed: {why}"),
                stop: "failed".to_owned(),
                by: Actor::default(),
            },
            failed,
        )
        .await;
        failure
    }

    /// Ends run r, whose session could not be opened, and puts the failure in front of the owner: his retry opens the
    /// session again.
    async fn fail_to_open(self: &Arc<Self>, r: &Arc<Run>, c: &Card, why: &str) {
        let f = Failed::new(Cause::NotOpened, why);
        if let Some(failure) = self.fail(r, &f).await {
            let retry = Retry {
                card_id: c.id.clone(),
                reopen: true,
                failure_ids: failure,
                ..Retry::default()
            };
            self.report_error(c, &f, Some(retry)).await;
        }
    }

    /// Records in run r's stream that its turn or session failed with why, and returns the events that do: the error, and
    /// the agent's last words when they only repeat it. Claude Code streams an API error, a usage limit for one, as the
    /// agent's own message and then fails the turn with the same text; what else the agent said is its work, and stays.
    pub async fn record_failure(self: &Arc<Self>, r: &Arc<Run>, why: &str) -> Vec<i64> {
        let mut failure = Vec::new();
        if let Some(last) = self.close_stream(r).await.filter(|last| repeats_failure(last, why)) {
            failure.push(last.id);
        }
        let (home, card_id) = {
            let s = r.state();
            (s.card.home.clone(), s.card.id.clone())
        };
        let added = self
            .add_event(Event {
                home,
                card_id,
                status: EventStatus::System(SystemNote::Error),
                text: why.to_owned(),
                ..Event::default()
            })
            .await;
        match added {
            Ok(e) => failure.push(e.id),
            // the inbox item the caller raises still says it
            Err(err) => tracing::error!(card = %r.card_id(), %err, "record failure"),
        }
        failure
    }

    /// Closes card's session, for why.
    pub fn close_run(&self, card_id: &str, why: &Cut) {
        if let Some(r) = self.run_of(card_id) {
            self.close_run_obj(&r, why);
        }
    }

    /// Closes this run, not whichever run its card has now: a newer one reopened meanwhile stays live. Its messages stay
    /// queued in the store; a turn in progress is cut short for why, in the background, as its process closes. False
    /// when the run had already closed.
    pub fn close_run_obj(&self, r: &Arc<Run>, why: &Cut) -> bool {
        if !r.close_box() {
            return false;
        }
        {
            let mut sh = self.shared();
            sh.tokens.remove(&r.token);
            sh.seats.revoke(&r.token); // the subagent types its session was offered end with it
        }
        let sess = r.state().sess.clone();
        let (o, run, why) = (self.me(), Arc::clone(r), why.clone());
        let ending = o.is_some_and(|o| self.background(move |_| async move { o.end_run(&run, &why).await }));
        if !ending {
            // shutting down: nothing more is recorded of it, and its process closes with it; Recover reads its card
            if let Some(sess) = sess {
                sess.close();
            }
        }
        true
    }

    /// Ends what run r, its mailbox closed, still holds: its process (a turn in progress cut short for why), its turns of
    /// its own, its background tasks and the child cards still at work in its process.
    pub async fn end_run(self: &Arc<Self>, r: &Arc<Run>, why: &Cut) {
        let sess = r.state().sess.clone();
        match sess {
            Some(sess) => {
                self.cut_turn_of(r, why).await;
                sess.close();
            }
            None => {
                r.end_asked(why); // a child card's requests wait on no session of its own
            }
        }
        let own: Vec<Arc<OwnTurn>> = {
            let mut s = r.state();
            let mut own: Vec<_> = s.own_turns.drain(..).collect();
            own.extend(s.own.clone());
            own
        };
        for t in own {
            t.end();
        }
        self.end_tasks(r, why).await;
        self.end_children(r, why).await;
        self.cut_claude_runs(&r.card_now(), why).await;
    }

    /// Closes the sessions of runs (of one chain) for why, oldest card first, and marks their cards stopped (a failed one
    /// stays failed) with activity, when given.
    pub async fn close_sessions(&self, mut runs: Vec<Arc<Run>>, activity: &str, why: &Cut) {
        runs.sort_by_key(|r| r.created_at());
        for r in runs {
            if !self.close_run_obj(&r, why) || !self.current(&r) {
                continue; // closed already, or replaced by a newer session
            }
            let activity = activity.to_owned();
            let now = self.now();
            if let Err(err) = self
                .update_card(&r.card_id(), move |c| {
                    if c.status() != &CardStatus::Failed {
                        c.set_status(CardStatus::Stopped, now);
                    }
                    if !activity.is_empty() {
                        c.activity.clone_from(&activity);
                    }
                })
                .await
            {
                tracing::error!(home = %r.home, card = %r.card_id(), %err, "stop card");
            }
        }
    }

    /// Closes the chain's sessions whose agents run background work, for why (the pause's `stop_background`).
    pub async fn stop_background(&self, chain: &str, why: &Cut) {
        let mut runs = Vec::new();
        for r in self.chain_runs(chain) {
            if !self.watching(&r).await.is_empty() {
                runs.push(r);
            }
        }
        self.close_sessions(runs, "paused", why).await;
    }

    /// Closes every session of the deleted chain, for why.
    pub fn drop_sessions(&self, chain: &str, why: &Cut) {
        for r in self.chain_runs(chain) {
            self.close_run_obj(&r, why);
        }
    }

    /// Cuts short the turns in progress in the chain, for why.
    pub async fn cut_turns(&self, chain: &str, why: &Cut) {
        for r in self.chain_runs(chain) {
            if r.busy() {
                self.cut_turn_of(&r, why).await;
            }
        }
    }

    /// Cuts short the turn card's session is in, for why.
    pub async fn cut_turn(&self, card_id: &str, why: &Cut) {
        if let Some(r) = self.run_of(card_id) {
            self.cut_turn_of(&r, why).await;
        }
    }

    /// Cancels r's turn for why and gives the agent `CANCEL_WAIT` to stop.
    pub async fn cut_turn_of(&self, r: &Run, why: &Cut) {
        match tokio::time::timeout(CANCEL_WAIT, r.cancel_turn(why)).await {
            Ok(Ok(())) => {}
            Ok(Err(err)) => tracing::warn!(agent = %r.id, %why, %err, "turn not cancelled"),
            Err(_) => tracing::warn!(agent = %r.id, %why, "turn not cancelled in time"),
        }
    }

    /// Sends the owner's message draft to its card as delivery says (`owner_says`), ending the draft with the send.
    pub async fn prompt(self: &Arc<Self>, req: Request, draft_id: String, delivery: Delivery) -> Result<Event> {
        let by = req.by.clone();
        self.act(req, "Prompt", move |o| async move {
            let d = o.draft_to_send(&draft_id, "").await?;
            let id = d.card_id.clone();
            let to = o.store.read(move |s| s.card(&id)).await?;
            let mut m = owner_message(&to.id, "Your message", d.text.trim(), by);
            m.data = with_images(m.data, &d.images);
            o.owner_says(
                &to,
                m,
                delivery,
                Records {
                    drafts: vec![d],
                    ..Records::default()
                },
            )
            .await
        })
        .await
    }

    /// Sends card `to` m, the owner's words (`owner_message`) by him or by the Assistant on his words, with w, as delivery
    /// says (ADR-0080): queued, read after the turn in progress; steered into that turn (queued when it ended meanwhile or
    /// the agent cannot take it, `steer`); or read next, the turn in progress ending.
    pub async fn owner_says(
        self: &Arc<Self>,
        to: &Card,
        mut m: Message,
        delivery: Delivery,
        w: Records,
    ) -> Result<Event> {
        let by = m.by.clone();
        m.steer = delivery == Delivery::Steer;
        let e = self.send(to, m, w).await?;
        let Some(r) = self.run_of(&to.id).filter(|_| delivery == Delivery::Interrupt) else {
            return Ok(e);
        };
        // the owner's message comes first in the queue, so it is read next
        if r.busy() {
            self.interrupt(&r, &by)
                .await
                .map_err(|err| Error::refused(format!("interrupt: {err}")))?;
        }
        Ok(e)
    }

    /// Takes back the owner's message event_id before any agent reads it (ADR-0080): it is deleted, and its words become
    /// its card's message draft again, which this answers. Refused once an agent took it or it was steered into a turn,
    /// for a message that is not the owner's own, and while that card's composer holds unsent words, which it would
    /// replace.
    pub async fn withdraw_prompt(self: &Arc<Self>, req: Request, event_id: i64) -> Result<Draft> {
        self.act(
            req,
            "WithdrawPrompt",
            move |o| async move { o.withdraw(event_id).await },
        )
        .await
    }

    async fn withdraw(&self, event_id: i64) -> Result<Draft> {
        let e = self.store.read(move |r| r.event(event_id)).await?;
        if e.kind() != EventKind::Prompt || e.by.as_ref().is_none_or(|by| by.kind != ActorKind::Owner) {
            return Err(Error::refused(format!("event {event_id} is not a message of yours")));
        }
        let d = Draft {
            id: new_id("d_"),
            home: e.home.clone(),
            text: e.text.clone(),
            images: images_in(e.data.as_ref()),
            card_id: e.card_id.clone(),
            updated_at: self.now(),
            ..Draft::default()
        };
        let (gone, d) = self
            .in_home(&e.home, || async {
                self.store
                    .update(move |tx| {
                        let held = tx
                            .drafts(&d.home)?
                            .iter()
                            .any(|x| x.card_id == d.card_id && x.mentor.is_empty());
                        if held {
                            return Ok(Err(Error::refused(
                                "the agent's message box holds words you have not sent: send or delete them first",
                            )));
                        }
                        let Some(gone) = tx.withdraw_prompt(event_id)? else {
                            return Ok(Err(Error::refused("the agent has read this message already")));
                        };
                        Ok::<_, store::Error>(Ok((gone, tx.put_draft(&d)?)))
                    })
                    .await?
            })
            .await?;
        self.notify.event_deleted(&gone);
        self.notify.draft_updated(&d);
        Ok(d)
    }
}

/// The span a turn of r's session runs in, the turn on message `turn` (for one the agent took on its own, the event that
/// opened it): what the session, its agent and the crates under them log in it says which chain, card, session and turn.
fn turn_span(r: &Run, turn: i64) -> tracing::Span {
    let s = r.state();
    let session = s.sess.as_ref().map(|sess| sess.id().to_owned()).unwrap_or_default();
    use ::telemetry::attr;
    tracing::info_span!(
        attr::TURN_SPAN,
        { attr::CHAIN_ID } = %r.home,
        { attr::CARD_ID } = %s.card.id,
        { attr::SESSION_ID } = %session,
        { attr::TURN } = turn
    )
}

/// Whether agent message e is the words of the failure, not of its work: the error contains them.
fn repeats_failure(e: &Event, failure: &str) -> bool {
    let text = e.text.trim();
    e.kind() == EventKind::Message && !text.is_empty() && failure.contains(text)
}

/// Whether a turn that ended with stop ended because the liveness rail cut it short (silenced): its agent answered the
/// cancel. One that ended on its own as the cancel was on its way did its work, and its end stands.
pub fn ended_by_silence(silenced: bool, stop: model::StopReason) -> bool {
    silenced && stop == model::StopReason::Cancelled
}

/// What a card whose turn a pause cut reads once its chain runs again.
const PAUSE_CONTINUES: &str = "[the chain was paused mid-turn and is now resumed] Continue where you stopped, then end your turn as you would have.";

/// Whether a turn that ended with stop, cut short for cut, ended because its chain paused: its agent answered the
/// pause's cancel. One that ended on its own as the cancel was on its way did its work, and its end stands.
pub fn ended_by_pause(cut: Option<&Cut>, stop: model::StopReason) -> bool {
    cut == Some(&Cut::Paused) && stop == model::StopReason::Cancelled
}

/// The data a turn's stop carries: how long the turn took.
pub fn took_data(d: Duration) -> Option<model::ForeignJson> {
    let ms = u64::try_from(d.as_millis()).unwrap_or(u64::MAX);
    Some(model::ForeignJson(serde_json::json!({ "durationMs": ms })))
}
