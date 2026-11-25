//! One session the runtime holds for the core, known by the core's session id. It is its launcher session's handler:
//! what the session does is written as numbered entries, which one sender gives the core in order, one call at a time,
//! until the core has them.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use agent::{BoxFuture, Update};
use runtimelink::Link;
use tokio::sync::{Notify, OnceCell, oneshot};
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use wire::runtime::{
    EntryBody, HeldSession, Limit, Moment, PermissionAnswer, PermissionRequest, SessionEntry, SessionOpened, Steered,
    Task, TaskOutput, to_core,
};

use crate::convert::{self, cut};
use crate::runtime::Runtime;
use crate::{lock, new_id};

/// The most one `session_events` call carries, encoded; one entry larger than it goes alone.
const BATCH_LIMIT: usize = 256 << 10;

/// How long the runtime gives Claude to start a session (the core gives up on an open after as long, and kills what it
/// gave up on), to take a cancel, and to exit once closed as the runtime ends.
const OPEN_WAIT: Duration = Duration::from_secs(180);
const CANCEL_WAIT: Duration = Duration::from_secs(5);
const CLOSE_WAIT: Duration = Duration::from_secs(10);

pub(crate) struct Session<L> {
    /// Itself, for the callbacks of its launcher session, which outlive the runtime's map of sessions.
    me: Weak<Session<L>>,
    r: Arc<Runtime<L>>,
    pub(crate) id: String,
    /// Cancelled when the session is closed while it opens: the open gives up.
    opening: CancellationToken,
    /// Cancelled once the open has its result.
    ready: CancellationToken,
    /// Cancelled once `on_exit` has written the exit entry.
    exited: CancellationToken,
    /// A new entry, or a new ack: the sender looks again.
    kick: Notify,
    /// The turns in flight: the exit entry waits for their ends.
    turns: TaskTracker,
    state: Mutex<State>,
}

struct State {
    /// How the open ended; none while it runs.
    opened: Option<Opened>,
    /// `close` has run: an open that ends now closes what it opened.
    closing: bool,
    /// The last entry numbered.
    seq: i64,
    /// The entries the core has not acked, `seq` ascending.
    outbox: Vec<SessionEntry>,
    /// `on_exit` has begun: no turn starts.
    ending: bool,
    /// The last entry is written (exit), or the open failed.
    done: bool,
    /// The agent works, as it last said.
    working: bool,
    prompts: HashSet<String>,
    /// Each steer by its id, from the first time it is asked: a replay waits for the first and answers as it did.
    steers: HashMap<String, Arc<OnceCell<Steered>>>,
    /// The permission requests waiting for the core, by request id.
    asks: HashMap<String, oneshot::Sender<PermissionAnswer>>,
}

struct Opened {
    /// The launcher's session; none when the open failed.
    sess: Option<Arc<dyn agent::Session>>,
    result: SessionOpened,
}

impl<L: Link> Session<L> {
    pub(crate) fn new(r: Arc<Runtime<L>>, id: String) -> Arc<Self> {
        let opening = r.life.child_token();
        Arc::new_cyclic(|me| Self {
            me: me.clone(),
            r,
            id,
            opening,
            ready: CancellationToken::new(),
            exited: CancellationToken::new(),
            kick: Notify::new(),
            turns: TaskTracker::new(),
            state: Mutex::new(State {
                opened: None,
                closing: false,
                seq: 0,
                outbox: Vec::new(),
                ending: false,
                done: false,
                working: false,
                prompts: HashSet::new(),
                steers: HashMap::new(),
                asks: HashMap::new(),
            }),
        })
    }

    fn state(&self) -> MutexGuard<'_, State> {
        lock(&self.state)
    }

    /// Starts the session in the launcher, within `OPEN_WAIT`, and writes how it ended as the opened entry: the core
    /// hands the session out once it has it. A failed open's opened entry is its last; a session closed while it opened
    /// is closed now.
    pub(crate) async fn open(self: Arc<Self>, spec: agent::Spec) {
        let card = spec.card_id.clone();
        let handler: Arc<dyn agent::Handler> = self.clone();
        let start = tokio::time::timeout(OPEN_WAIT, self.r.cfg.launcher.start(spec, handler));
        let started = tokio::select! {
            started = start => started.unwrap_or_else(|_| {
                Err(agent::Error::TurnFailed(format!("Claude did not start within {OPEN_WAIT:?}")))
            }),
            () = self.opening.cancelled() => Err(agent::Error::TurnFailed("the session was closed as it opened".to_owned())),
        };
        let result = convert::opened(&started);
        if let Err(e) = &started {
            tracing::warn!(session = %self.id, card = %card, error = %e, "open a session");
        }
        let sess: Option<Arc<dyn agent::Session>> = started.ok().map(Arc::from);
        let closing = {
            let mut state = self.state();
            if sess.is_none() {
                // What a session that did not open said is no one's to read.
                state.outbox.clear();
            }
            state.opened = Some(Opened {
                sess: sess.clone(),
                result: result.clone(),
            });
            state.closing
        };
        self.ready.cancel();
        self.write(EntryBody::Opened { opened: result });
        match sess {
            None => {
                self.state().done = true;
                self.kick.notify_one();
            }
            Some(sess) if closing => sess.close(),
            Some(_) => {}
        }
    }

    /// Says the session opened and the core has not yet acked its exit.
    pub(crate) fn is_open(&self) -> bool {
        self.state().opened.as_ref().is_some_and(|o| o.sess.is_some())
    }

    /// The launcher's session, once open. It does not wait for an open: the core sends nothing to a session before it
    /// has its opened entry, which follows `ready`.
    fn live(&self) -> Result<Arc<dyn agent::Session>, String> {
        match &self.state().opened {
            None => Err("the session is still opening".to_owned()),
            Some(Opened { sess: Some(sess), .. }) => Ok(Arc::clone(sess)),
            Some(Opened { result, .. }) => Err(format!("the session did not open: {}", result.refused)),
        }
    }

    pub(crate) fn held_as(&self) -> HeldSession {
        let state = self.state();
        HeldSession {
            session_id: self.id.clone(),
            last_seq: state.seq,
            working: state.working,
        }
    }

    /// Starts a turn on `prompt`, once per `prompt_id`, and returns: the turn runs on, and its end is a `turnEnded` entry.
    /// The launcher refuses a prompt while a turn runs, and that refusal is the turn's end.
    pub(crate) fn prompt(self: &Arc<Self>, prompt_id: String, prompt: model::Prompt) -> Result<(), String> {
        let sess = self.live()?;
        let mut state = self.state();
        if state.ending || !state.prompts.insert(prompt_id.clone()) {
            // Taken already, or the session ends: its exit ends the core's prompt.
            return Ok(());
        }
        let s = Arc::clone(self);
        // Spawned under the lock, so `on_exit`, which sets `ending` under it, waits for this turn.
        self.turns.spawn(async move {
            let ended = tokio::select! {
                ended = sess.prompt(prompt) => ended,
                () = s.r.life.cancelled() => Err(agent::Error::TurnFailed("the AI runtime is ending".to_owned())),
            };
            s.write(EntryBody::TurnEnded {
                turn: convert::turn_end(prompt_id, ended),
            });
        });
        drop(state);
        Ok(())
    }

    /// Gives the running turn `prompt`, once per `steer_id`. The steer is marked under the lock before it is given, so a
    /// replay that comes while the first is given waits for it and answers as it did; one whose first failed gives it.
    pub(crate) async fn steer(&self, steer_id: String, prompt: model::Prompt) -> Result<Steered, String> {
        let sess = self.live()?;
        let once = Arc::clone(self.state().steers.entry(steer_id).or_default());
        let out = once
            .get_or_try_init(|| async move {
                let steered = sess.steer(prompt).await.map_err(|e| e.to_string())?;
                Ok::<_, String>(Steered { steered })
            })
            .await?;
        Ok(out.clone())
    }

    /// Ends one task of the open session's process; the task's end follows as its task entry.
    pub(crate) async fn stop_task(&self, task_id: String) -> Result<(), String> {
        self.live()?.stop_task(task_id).await.map_err(|e| e.to_string())
    }

    /// Counts what fills the open session's context.
    pub(crate) async fn context(&self) -> Result<model::SessionContext, String> {
        self.live()?.context().await.map_err(|e| e.to_string())
    }

    /// Reads the end of what one background task of the open session's process has written.
    pub(crate) async fn task_output(&self, task_id: String) -> Result<TaskOutput, String> {
        self.live()?.task_output(task_id).await.map_err(|e| e.to_string())
    }

    /// Moves the open session onto the login of account; its process and turn go on (ADR-0074).
    pub(crate) async fn move_to(&self, account: String) -> Result<(), String> {
        self.live()?.move_to(account).await.map_err(|e| e.to_string())
    }

    /// Asks Claude to cut the running turn, and gives it `CANCEL_WAIT` to take the ask; the turn's end follows as its
    /// `turnEnded` entry. It holds no other call while Claude takes it.
    pub(crate) fn cancel(&self) -> Result<(), String> {
        let sess = self.live()?;
        let id = self.id.clone();
        tokio::spawn(async move {
            match tokio::time::timeout(CANCEL_WAIT, sess.cancel()).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => tracing::warn!(session = %id, error = %e, "cancel a turn"),
                Err(_) => tracing::warn!(session = %id, "Claude did not take a cancel in time"),
            }
        });
        Ok(())
    }

    /// Closes the launcher's session, once, without waiting: an open in progress gives up, and the session it opens
    /// anyway is closed by `open`. `on_exit` follows for a session that opened.
    pub(crate) fn close(&self) {
        let sess = {
            let mut state = self.state();
            if state.closing {
                return;
            }
            state.closing = true;
            state.opened.as_ref().and_then(|o| o.sess.clone())
        };
        self.opening.cancel();
        if let Some(sess) = sess {
            sess.close();
        }
    }

    /// Closes the session and returns once it has exited, or after `CLOSE_WAIT`.
    pub(crate) async fn close_and_wait(&self) {
        self.close();
        let exit = async {
            self.ready.cancelled().await;
            if self.is_open() {
                self.exited.cancelled().await;
            }
        };
        if tokio::time::timeout(CLOSE_WAIT, exit).await.is_err() {
            tracing::warn!(session = %self.id, wait = ?CLOSE_WAIT, "a session did not exit in time");
        }
    }

    /// Hands the core's decision to the permission request that waits for it; one answered already, or no longer
    /// waiting, changes nothing.
    pub(crate) fn answer(&self, request_id: &str, d: PermissionAnswer) {
        if let Some(tx) = self.state().asks.remove(request_id) {
            tx.send(d).ok();
        }
    }

    /// Numbers `body`, an entry of the session's own agent, stamps it and puts it in the outbox for the sender.
    fn write(&self, body: EntryBody) {
        self.write_by(String::new(), body);
    }

    /// Numbers `body`, what the agent `agent_id` did (empty: the session's own), stamps it and puts it in the outbox for
    /// the sender.
    fn write_by(&self, agent_id: String, body: EntryBody) {
        let body = match body {
            EntryBody::Message { text } => EntryBody::Message { text: cut(text) },
            EntryBody::Thought { text } => EntryBody::Thought { text: cut(text) },
            EntryBody::Exit { exit } => EntryBody::Exit { exit: cut(exit) },
            other => other,
        };
        let at = self.r.now();
        {
            let mut state = self.state();
            state.seq = state.seq.saturating_add(1);
            let seq = state.seq;
            state.outbox.push(SessionEntry {
                seq,
                agent_id,
                at,
                body,
            });
        }
        self.kick.notify_one();
    }

    /// Drops the entries the core has, up to `acked`.
    pub(crate) fn ack(&self, acked: i64) {
        self.state().outbox.retain(|e| e.seq > acked);
        self.kick.notify_one();
    }

    /// The next entries to send, at most `BATCH_LIMIT` encoded (one at least), and whether nothing is left to send for
    /// good.
    fn batch(&self) -> (Vec<SessionEntry>, bool) {
        let state = self.state();
        let mut out = Vec::new();
        let mut size = 0usize;
        for e in &state.outbox {
            let n = match link::Payload::encode(e) {
                Ok(b) => b.as_bytes().len(),
                Err(err) => {
                    tracing::error!(session = %self.id, seq = e.seq, error = %err, "an entry that cannot be sent");
                    continue;
                }
            };
            if !out.is_empty() && size.saturating_add(n) > BATCH_LIMIT {
                break;
            }
            size = size.saturating_add(n);
            out.push(e.clone());
        }
        let finished = state.outbox.is_empty() && state.done;
        (out, finished)
    }

    /// The session's one sender: it gives the core every entry after the last it acked, one call in flight, and sends
    /// again what a dropped call lost. It ends once the core has the last entry (exit, or a failed open's opened), or
    /// with the runtime, and the runtime lets the session go.
    pub(crate) async fn send(self: Arc<Self>) {
        loop {
            let (entries, finished) = self.batch();
            if finished {
                break;
            }
            if entries.is_empty() {
                tokio::select! {
                    () = self.kick.notified() => continue,
                    () = self.r.life.cancelled() => break,
                }
            }
            let Some(conn) = self.r.connection().await else {
                break;
            };
            let events = to_core::client::session_events(self.id.clone(), entries);
            match conn.link.call(events).await {
                Ok(acked) => self.ack(acked.acked),
                Err(f) => {
                    tracing::debug!(session = %self.id, error = %f, "send a session's entries");
                    if !self.r.pause(&conn).await {
                        break;
                    }
                }
            }
        }
        self.r.forget(&self);
    }
}

/// Withdraws a permission request whose wait the agent gave up on.
struct Withdraw<'a, L: Link> {
    s: &'a Session<L>,
    id: String,
    answered: bool,
}

impl<L: Link> Drop for Withdraw<'_, L> {
    fn drop(&mut self) {
        self.s.state().asks.remove(&self.id);
        if !self.answered {
            self.s.write(EntryBody::PermissionWithdrawn {
                request_id: self.id.clone(),
            });
        }
    }
}

impl<L: Link> Session<L> {
    /// Writes what the agent `agent_id` (empty: the session's own) did.
    fn update(&self, agent_id: String, u: Update) {
        let body = match u {
            Update::MessageChunk(text) => EntryBody::Message { text },
            Update::ThoughtChunk(text) => EntryBody::Thought { text },
            Update::ToolCall(tool) => EntryBody::Tool {
                tool: convert::tool_call(tool),
            },
            Update::Plan(plan) => EntryBody::Plan {
                plan: Some(convert::plan(plan)),
            },
            Update::Usage(usage) => EntryBody::Usage { usage },
            Update::Note(note) => EntryBody::Note { note },
        };
        self.write_by(agent_id, body);
    }

    /// Writes a task the agent `agent_id` (empty: the session's own) started: a subagent it spawned is the one that
    /// names it.
    fn task(&self, agent_id: String, t: Task) {
        self.write_by(agent_id, EntryBody::Task { task: convert::task(t) });
    }

    /// Asks the core, once connected, what the agent `agent_id` (empty: the session's own) reads beside moment m, after
    /// the entries it wrote before: a call that fails adds nothing. The agent's wait bounds it.
    fn moment(&self, agent_id: String, m: Moment) -> BoxFuture<'_, Option<String>> {
        Box::pin(async move {
            let after_seq = self.state().seq;
            let conn = self.r.connection().await?;
            let asked = conn
                .link
                .call(to_core::client::session_moment(self.id.clone(), agent_id, after_seq, m))
                .await;
            asked.ok().map(|t| t.text).filter(|t| !t.is_empty())
        })
    }
}

/// The handler of one subagent of a session: it writes the same entries, naming the subagent. A subagent has no process
/// of its own: its parent's says what it loaded, whether it works and when it exits, so those callbacks never come here
/// and write nothing.
struct Subagent<L> {
    s: Arc<Session<L>>,
    id: String,
}

impl<L: Link> agent::Handler for Subagent<L> {
    fn on_update(&self, u: Update) {
        self.s.update(self.id.clone(), u);
    }
    fn on_permission(&self, req: PermissionRequest) -> BoxFuture<'_, PermissionAnswer> {
        self.s.on_permission(req)
    }
    fn on_init(&self, _init: model::SessionInit) {}
    fn on_task(&self, t: Task) {
        self.s.task(self.id.clone(), t);
    }
    fn on_working(&self, _working: bool) {}
    fn on_exit(&self, _err: Option<agent::Exit>) {}
    fn on_subagent(&self, agent_id: &str) -> Option<Arc<dyn agent::Handler>> {
        Some(Arc::new(Subagent {
            s: Arc::clone(&self.s),
            id: agent_id.to_owned(),
        }))
    }
    fn on_moment(&self, m: Moment) -> BoxFuture<'_, Option<String>> {
        self.s.moment(self.id.clone(), m)
    }
}

impl<L: Link> agent::Handler for Session<L> {
    fn on_update(&self, u: Update) {
        self.update(String::new(), u);
    }

    /// Asks the core (again on every connection until it acks the ask) and waits for its answer, a call of its own. A
    /// wait the agent gives up on (this future dropped) is withdrawn.
    fn on_permission(&self, req: PermissionRequest) -> BoxFuture<'_, PermissionAnswer> {
        Box::pin(async move {
            let id = new_id();
            let (tx, rx) = oneshot::channel();
            self.state().asks.insert(id.clone(), tx);
            let mut withdraw = Withdraw {
                s: self,
                id: id.clone(),
                answered: false,
            };
            // The entries the agent wrote before it asked: the core asks after them.
            let after_seq = self.state().seq;
            let request = convert::permission_request(req);
            while let Some(conn) = self.r.connection().await {
                let ask = to_core::client::permission_asked(self.id.clone(), id.clone(), request.clone(), after_seq);
                if conn.link.call(ask).await.is_ok() || !self.r.pause(&conn).await {
                    break;
                }
            }
            let decision = tokio::select! {
                d = rx => d.ok(),
                () = self.r.life.cancelled() => None,
            };
            match decision {
                Some(d) => {
                    withdraw.answered = true;
                    d
                }
                None => PermissionAnswer::Deny {
                    message: "the request was withdrawn".to_owned(),
                },
            }
        })
    }

    /// Asks the core what the agent must do before its turn ends, once connected (the turn's end waits for the core
    /// anyway): a call that fails lets the turn end.
    fn on_stop(&self) -> BoxFuture<'_, Option<String>> {
        Box::pin(async move {
            let conn = self.r.connection().await?;
            let asked = conn.link.call(to_core::client::session_stopping(self.id.clone())).await;
            asked.ok().map(|s| s.first).filter(|f| !f.is_empty())
        })
    }

    fn on_moment(&self, m: Moment) -> BoxFuture<'_, Option<String>> {
        self.moment(String::new(), m)
    }

    fn on_init(&self, init: model::SessionInit) {
        self.write(EntryBody::Init {
            init: convert::session_init(init),
        });
    }

    fn on_context(&self, context: model::SessionContext) {
        self.write(EntryBody::Context { context });
    }

    fn on_task(&self, t: Task) {
        self.task(String::new(), t);
    }

    /// The handler of a subagent of the session: it writes the same entries, naming the subagent.
    fn on_subagent(&self, agent_id: &str) -> Option<Arc<dyn agent::Handler>> {
        let s = self.me.upgrade()?;
        Some(Arc::new(Subagent {
            s,
            id: agent_id.to_owned(),
        }))
    }

    fn on_working(&self, working: bool) {
        self.state().working = working;
        self.write(EntryBody::Working { working });
    }

    fn on_limit(&self, limit: Limit) {
        self.write(EntryBody::Limit { limit });
    }

    /// Writes the session's last entry, after the turn in flight has written its end.
    fn on_exit(&self, err: Option<agent::Exit>) {
        self.state().ending = true;
        let Some(s) = self.me.upgrade() else {
            return;
        };
        let exit = err.map(|e| e.to_string()).unwrap_or_default();
        tokio::spawn(async move {
            s.turns.close();
            s.turns.wait().await;
            s.write(EntryBody::Exit { exit });
            s.state().done = true;
            s.exited.cancel();
            s.kick.notify_one();
        });
    }
}
