//! One session the runtime runs for the core, known to the runtime by the core's session id and to the card by Claude's
//! (its resume id). What the runtime reports of it arrives as numbered entries; each reaches the handler once, in order,
//! on a task of the session's own (`hand_over`), never on the call that brought it.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use agent::{BoxFuture, Handler, Subagents, Update};
use model::StopReason;
use tokio::sync::{oneshot, watch};
use tokio_util::sync::CancellationToken;
use wire::runtime::{
    EntryBody, Moment, PermissionRequest, SessionEntry, SessionOpened, TASK_SUBAGENT, TaskOutput, TurnEnd, to_runtime,
};

use crate::convert::Ending;
use crate::launcher::{Launcher, Runtime};
use crate::port::Link;
use crate::{lock, new_id};

pub(crate) struct Session<L> {
    l: Launcher<L>,
    rt: Arc<Runtime<L>>,
    pub(crate) id: String,
    h: Arc<dyn Handler>,
    /// The handlers of its subagents, `h` the root; only `hand_over` and `decide` use them.
    agents: Subagents,
    /// The last entry handed to the handler: a permission request is asked once the entries before it are.
    handled: watch::Sender<i64>,
    /// Claude's id of the conversation, and whether it replaced one that was lost: set before the session is handed out.
    resumed: OnceLock<(String, bool)>,
    /// Cancelled when the session ends.
    life: CancellationToken,
    /// The runtime's opened entry: how its open ended.
    opened_with: watch::Sender<Option<SessionOpened>>,
    /// Cancelled once the exit entry has arrived.
    exited: CancellationToken,
    state: Mutex<State>,
}

struct State {
    /// The start handed the session out: the runtime's end ends it, and the handler hears of its end.
    opened: bool,
    /// The last entry received.
    last_seq: i64,
    /// Entries received and not yet handed to the handler, in order.
    inbox: VecDeque<SessionEntry>,
    /// The runtime ended under the session: its end, once the inbox is handed over.
    gone: Option<Ending>,
    /// A `hand_over` task runs.
    handing: bool,
    /// The prompts waiting for their turn's end, by prompt id.
    turns: HashMap<String, Turn>,
    /// Every permission request asked, by request id; cancelling its token ends its wait.
    asked: HashMap<String, CancellationToken>,
    /// Why the session ended; none while it runs.
    ended: Option<Ending>,
}

type TurnResult = Result<StopReason, agent::Error>;

struct Turn {
    /// The turn's end, sent once.
    done: oneshot::Sender<TurnResult>,
    /// Resolves once the prompt has returned.
    returned: oneshot::Receiver<()>,
}

impl<L: Link> Session<L> {
    pub(crate) fn new(l: Launcher<L>, rt: Arc<Runtime<L>>, id: String, h: Arc<dyn Handler>) -> Arc<Self> {
        Arc::new(Self {
            l,
            rt,
            id,
            agents: Subagents::new(Arc::clone(&h)),
            h,
            handled: watch::channel(0).0,
            resumed: OnceLock::new(),
            life: CancellationToken::new(),
            opened_with: watch::channel(None).0,
            exited: CancellationToken::new(),
            state: Mutex::new(State {
                opened: false,
                last_seq: 0,
                inbox: VecDeque::new(),
                gone: None,
                handing: false,
                turns: HashMap::new(),
                asked: HashMap::new(),
                ended: None,
            }),
        })
    }

    fn state(&self) -> MutexGuard<'_, State> {
        lock(&self.state)
    }

    pub(crate) fn resumed(&self, resume_id: String, lost: bool) {
        self.resumed.set((resume_id, lost)).ok();
    }

    pub(crate) fn is_opened(&self) -> bool {
        self.state().opened
    }

    /// Marks the session handed out, unless it ended as it opened.
    pub(crate) fn mark_opened(&self) -> Result<(), agent::Error> {
        let mut state = self.state();
        if let Some(ended) = &state.ended {
            return Err(ended.turn_error());
        }
        state.opened = true;
        Ok(())
    }

    /// The runtime's opened entry, which says how its open ended; an error when the session or its runtime ended first.
    pub(crate) async fn until_opened(&self) -> Result<SessionOpened, agent::Error> {
        let mut opened = self.opened_with.subscribe();
        let wait = async {
            opened
                .wait_for(Option::is_some)
                .await
                .ok()
                .and_then(|opened| (*opened).clone())
        };
        tokio::select! {
            opened = wait => opened.ok_or_else(|| agent::Error::TurnFailed("the session ended as it opened".to_owned())),
            () = self.life.cancelled() => Err(self.ended_with().unwrap_or(Ending::Exited(None)).turn_error()),
            () = self.rt.gone.cancelled() => Err(agent::Error::HostRestarted),
        }
    }

    fn ended_with(&self) -> Option<Ending> {
        self.state().ended.clone()
    }

    /// Kills a session whose start gave up: the runtime may be opening it, and nobody would close it. It waits for no
    /// exit: a session whose open the kill cuts has none.
    pub(crate) fn abandon(self: &Arc<Self>) {
        self.l.drop_session(&self.rt, self);
        self.life.cancel();
        let s = Arc::clone(self);
        tokio::spawn(async move {
            match tokio::time::timeout(s.l.inner.kill_wait, s.kill()).await {
                Ok(Ok(())) | Ok(Err(crate::launcher::CallError::Gone(_))) => {}
                Ok(Err(crate::launcher::CallError::Link(f))) => {
                    tracing::warn!(session = %s.id, error = %f, "could not kill a session whose start gave up");
                }
                Err(_) => {
                    tracing::warn!(session = %s.id, "the kill of a session whose start gave up was not answered")
                }
            }
        });
    }

    /// Asks the runtime to end the session; it takes the ask at once, and the exit entry follows.
    async fn kill(&self) -> Result<(), crate::launcher::CallError> {
        self.l
            .call(&self.rt, to_runtime::client::session_kill(self.id.clone()))
            .await
            .map(|_| ())
    }

    /// Starts a turn in the runtime and waits for the `turnEnded` entry that names it.
    async fn prompt(&self, prompt: model::Prompt) -> TurnResult {
        let id = new_id();
        let (done, ended) = oneshot::channel();
        // Dropped when this prompt returns, however it does: the session's end waits for it.
        let (_returned, returned) = oneshot::channel::<()>();
        {
            let mut state = self.state();
            if let Some(ended) = &state.ended {
                return Err(ended.turn_error());
            }
            state.turns.insert(id.clone(), Turn { done, returned });
        }
        let _forget = Forget { s: self, id: &id };
        if let Err(e) = self
            .l
            .call(
                &self.rt,
                to_runtime::client::session_prompt(self.id.clone(), id.clone(), prompt),
            )
            .await
        {
            return Err(e.agent("prompt"));
        }
        ended.await.unwrap_or(Err(agent::Error::Ended(None)))
    }

    /// Gives the running turn more words; false when no turn ran to take them.
    async fn steer(&self, prompt: model::Prompt) -> Result<bool, agent::Error> {
        self.l
            .call(
                &self.rt,
                to_runtime::client::session_steer(self.id.clone(), new_id(), prompt),
            )
            .await
            .map(|s| s.steered)
            .map_err(|e| e.agent("steer"))
    }

    /// Asks the runtime to cut the running turn, and returns once it has taken the ask; the prompt returns with the
    /// `turnEnded` that follows.
    async fn cancel(&self) -> Result<(), agent::Error> {
        self.l
            .call(&self.rt, to_runtime::client::session_cancel(self.id.clone()))
            .await
            .map(|_| ())
            .map_err(|e| e.agent("cancel"))
    }

    /// Asks the runtime to end one task of the session's process; the task's end follows as its task entry.
    async fn stop_task(&self, task_id: String) -> Result<(), agent::Error> {
        self.l
            .call(
                &self.rt,
                to_runtime::client::session_stop_task(self.id.clone(), task_id),
            )
            .await
            .map(|_| ())
            .map_err(|e| e.agent("stop task"))
    }

    /// Asks the runtime to count what fills the session's context.
    async fn context(&self) -> Result<model::SessionContext, agent::Error> {
        self.l
            .call(&self.rt, to_runtime::client::session_context(self.id.clone()))
            .await
            .map_err(|e| e.agent("context"))
    }

    /// Asks the runtime for the end of what one background task of the session's process has written.
    async fn task_output(&self, task_id: String) -> Result<TaskOutput, agent::Error> {
        self.l
            .call(
                &self.rt,
                to_runtime::client::session_task_output(self.id.clone(), task_id),
            )
            .await
            .map_err(|e| e.agent("task output"))
    }

    /// Asks the runtime to move the running session onto the login of account (ADR-0074).
    async fn move_to(&self, account: String) -> Result<(), agent::Error> {
        self.l
            .call(&self.rt, to_runtime::client::session_move(self.id.clone(), account))
            .await
            .map(|_| ())
            .map_err(|e| e.agent("move"))
    }

    /// Kills the session in the runtime and waits for its exit entry: its process group has exited. A runtime that does
    /// not get it there within the kill wait is killed whole by its supervisor, which ends the session with it. A session
    /// gone with its runtime is closed already.
    fn close(self: &Arc<Self>) {
        if self.ended_with().is_some() {
            return;
        }
        let s = Arc::clone(self);
        tokio::spawn(async move {
            let killed = tokio::time::timeout(s.l.inner.kill_wait, async {
                s.kill().await?;
                s.until_exited().await
            })
            .await;
            let why = match killed {
                Ok(Ok(())) | Ok(Err(crate::launcher::CallError::Gone(_))) => return,
                Ok(Err(crate::launcher::CallError::Link(f))) => f.to_string(),
                Err(_) => "no exit entry in time".to_owned(),
            };
            if s.l.kill_runtime(&s.rt) {
                tracing::warn!(session = %s.id, runtime = %s.rt.id, error = %why, "the AI runtime did not end a session in time: it was killed");
            } else {
                tracing::warn!(session = %s.id, error = %why, "the AI runtime did not end a session in time");
            }
        });
    }

    /// Waits for the session's exit entry, or for its runtime's end.
    async fn until_exited(&self) -> Result<(), crate::launcher::CallError> {
        tokio::select! {
            () = self.exited.cancelled() => Ok(()),
            () = self.rt.gone.cancelled() => Err(crate::launcher::CallError::Gone(self.rt.id.clone())),
        }
    }

    /// Takes the entries after the last one received and says the last one received. It does not wait for the handler:
    /// the core answers `session_events` one call at a time for every session, and a callback that closes or cancels a
    /// session waits on entries that come through that queue. `hand_over` gives them to the handler, in order.
    pub(crate) fn receive(self: &Arc<Self>, entries: Vec<SessionEntry>) -> i64 {
        let mut state = self.state();
        for e in entries {
            if e.seq <= state.last_seq {
                continue;
            }
            state.last_seq = e.seq;
            if matches!(e.body, EntryBody::Exit { .. }) {
                self.exited.cancel();
            }
            state.inbox.push_back(e);
        }
        self.hand_over_locked(&mut state);
        state.last_seq
    }

    pub(crate) fn acked(&self) -> i64 {
        self.state().last_seq
    }

    /// Starts `hand_over` unless it runs.
    fn hand_over_locked(self: &Arc<Self>, state: &mut State) {
        if state.handing || (state.inbox.is_empty() && state.gone.is_none()) {
            return;
        }
        state.handing = true;
        tokio::spawn(Arc::clone(self).hand_over());
    }

    /// Hands the inbox to the handler, oldest first, then the end of the session's runtime when it ended, and returns once
    /// nothing is left. Only one runs at a time, so the handler hears one thing at a time, in order.
    async fn hand_over(self: Arc<Self>) {
        loop {
            let next = {
                let mut state = self.state();
                if let Some(e) = state.inbox.pop_front() {
                    Next::Entry(Box::new(e))
                } else if let Some(gone) = state.gone.take() {
                    Next::Gone(gone)
                } else {
                    state.handing = false;
                    return;
                }
            };
            match next {
                Next::Entry(e) => {
                    let seq = e.seq;
                    self.handle(*e).await;
                    self.handled.send_replace(seq);
                }
                Next::Gone(why) => self.end(why).await,
            }
        }
    }

    /// Hands one entry to the handler of the agent that did it (`agent_id`: a subagent's, or the session's own); what a
    /// subagent with no handler did is dropped. Only `hand_over` calls it.
    async fn handle(self: &Arc<Self>, e: SessionEntry) {
        if self.ended_with().is_some() {
            return;
        }
        if let EntryBody::Task { task } = &e.body
            && task.kind == TASK_SUBAGENT
        {
            self.agents.owns(&e.agent_id, &task.id);
        }
        let Some(h) = self.agents.handler(&e.agent_id) else {
            return;
        };
        match e.body {
            EntryBody::Opened { opened } => {
                // A session opens once.
                self.opened_with.send_if_modified(|was| {
                    if was.is_some() {
                        return false;
                    }
                    *was = Some(opened);
                    true
                });
            }
            EntryBody::Init { init } => h.on_init(init),
            EntryBody::Context { context } => h.on_context(context),
            EntryBody::Message { text } => h.on_update(Update::MessageChunk(text)),
            EntryBody::Thought { text } => h.on_update(Update::ThoughtChunk(text)),
            EntryBody::Tool { tool } => h.on_update(Update::ToolCall(tool)),
            EntryBody::Plan { plan } => {
                if let Some(plan) = plan {
                    h.on_update(Update::Plan(plan));
                }
            }
            EntryBody::Usage { usage } => h.on_update(Update::Usage(usage)),
            EntryBody::Note { note } => h.on_update(Update::Note(note)),
            EntryBody::Task { task } => h.on_task(task),
            EntryBody::Working { working } => h.on_working(working),
            EntryBody::TurnEnded { turn } => self.turn_ended(turn),
            EntryBody::Limit { limit } => h.on_limit(limit),
            EntryBody::PermissionWithdrawn { request_id } => self.withdraw(&request_id),
            EntryBody::Exit { exit } => {
                let why = if exit.is_empty() { None } else { Some(exit) };
                self.end(Ending::Exited(why)).await;
            }
        }
    }

    /// Ends the prompt the entry names. A permission request outlives the turn: one a subagent in the background asks
    /// still waits once the turn that spawned it has ended, and the runtime withdraws a request its agent no longer waits
    /// on (`PermissionWithdrawn`), which alone ends a wait before the session does.
    fn turn_ended(&self, te: TurnEnd) {
        // None: a prompt whose caller stopped waiting.
        let Some(turn) = self.state().turns.remove(&te.prompt_id) else {
            return;
        };
        let result = if let Some(failure) = te.failure {
            Err(agent::Error::Failure(failure))
        } else if !te.error.is_empty() {
            Err(agent::Error::TurnFailed(te.error))
        } else {
            Ok(te.stop.unwrap_or(StopReason::EndTurn))
        };
        turn.done.send(result).ok();
    }

    /// What the session's own agent must do before its turn ends, as its handler says.
    pub(crate) async fn stopping(&self) -> Option<String> {
        self.agents.handler("")?.on_stop().await
    }

    /// What the agent agent_id (empty: the session's own) reads beside moment m, as its handler says, once every entry the
    /// session wrote before it (to after_seq) is handed over: a subagent's start finds the subagent its task made. None
    /// once the session ends.
    pub(crate) async fn moment(&self, agent_id: &str, after_seq: i64, m: Moment) -> Option<String> {
        let mut handled = self.handled.subscribe();
        tokio::select! {
            // The sender lives as long as the session.
            _ = handled.wait_for(|seq| *seq >= after_seq) => {}
            () = self.life.cancelled() => return None,
        }
        let h = self.agents.handler(agent_id)?;
        tokio::select! {
            told = h.on_moment(m) => told,
            () = self.life.cancelled() => None,
        }
    }

    /// Hands a permission request, once every entry the session wrote before it is handed over, to the handler of the
    /// agent that asks (`Subagents::asker`) on a task of its own, and answers the runtime with its decision in a call of
    /// its own: the ask is acknowledged at once. A request asked again is not asked twice.
    pub(crate) fn ask(self: &Arc<Self>, request_id: String, req: PermissionRequest, after_seq: i64) {
        let token = {
            let mut state = self.state();
            if state.asked.contains_key(&request_id) || state.ended.is_some() {
                return;
            }
            let token = self.life.child_token();
            state.asked.insert(request_id.clone(), token.clone());
            token
        };
        let s = Arc::clone(self);
        tokio::spawn(async move { s.decide(token, request_id, after_seq, req).await });
    }

    async fn decide(&self, token: CancellationToken, request_id: String, after_seq: i64, req: PermissionRequest) {
        let mut handled = self.handled.subscribe();
        tokio::select! {
            // The sender lives as long as the session.
            _ = handled.wait_for(|seq| *seq >= after_seq) => {}
            () = token.cancelled() => return,
        }
        let asker = self.agents.asker(&req.agent_id);
        // The session no longer waits for the answer: the handler's wait is dropped.
        let decision = tokio::select! {
            d = asker.on_permission(req) => d,
            () = token.cancelled() => return,
        };
        if token.is_cancelled() {
            return;
        }
        let answer = to_runtime::client::permission_answered(self.id.clone(), request_id.clone(), decision);
        let answered = tokio::select! {
            answered = self.l.call(&self.rt, answer) => answered,
            () = self.life.cancelled() => return,
        };
        if let Err(crate::launcher::CallError::Link(f)) = answered {
            tracing::warn!(session = %self.id, request = %request_id, error = %f, "could not answer a permission request");
        }
    }

    fn withdraw(&self, request_id: &str) {
        let token = self.state().asked.get(request_id).cloned();
        if let Some(token) = token {
            token.cancel();
        }
    }

    /// Ends the session from outside its entries, its runtime having ended: after the entries received before, on
    /// `hand_over`, so the caller never waits for the handler.
    pub(crate) fn finish(self: &Arc<Self>, why: Ending) {
        let mut state = self.state();
        if state.gone.is_none() {
            state.gone = Some(why);
        }
        self.hand_over_locked(&mut state);
    }

    /// Ends the session for `why`: the prompt still open returns first, every permission wait ends, and the handler
    /// hears `on_exit` last. A session the start has not handed out is not the handler's to hear of: its start fails
    /// instead. Only `hand_over` calls it.
    async fn end(&self, why: Ending) {
        let (turns, opened) = {
            let mut state = self.state();
            if state.ended.is_some() {
                return;
            }
            state.ended = Some(why.clone());
            (std::mem::take(&mut state.turns), state.opened)
        };
        let mut returned = Vec::with_capacity(turns.len());
        for (_, turn) in turns {
            turn.done.send(Err(why.turn_error())).ok();
            returned.push(turn.returned);
        }
        for r in returned {
            r.await.ok();
        }
        self.life.cancel();
        self.l.drop_session(&self.rt, self);
        if opened {
            self.h.on_exit(why.exit());
        }
    }
}

enum Next {
    Entry(Box<SessionEntry>),
    Gone(Ending),
}

/// Takes a prompt's turn out of the session when the prompt stops waiting.
struct Forget<'a, L> {
    s: &'a Session<L>,
    id: &'a str,
}

impl<L> Drop for Forget<'_, L> {
    fn drop(&mut self) {
        lock(&self.s.state).turns.remove(self.id);
    }
}

/// The session as the core's session port hands it out.
pub(crate) struct Handle<L>(pub(crate) Arc<Session<L>>);

impl<L: Link> agent::Session for Handle<L> {
    fn id(&self) -> &str {
        self.0.resumed.get().map_or("", |(id, _)| id.as_str())
    }

    fn prompt(&self, prompt: model::Prompt) -> BoxFuture<'_, Result<StopReason, agent::Error>> {
        Box::pin(self.0.prompt(prompt))
    }

    fn steer(&self, prompt: model::Prompt) -> BoxFuture<'_, Result<bool, agent::Error>> {
        Box::pin(self.0.steer(prompt))
    }

    fn cancel(&self) -> BoxFuture<'_, Result<(), agent::Error>> {
        Box::pin(self.0.cancel())
    }

    fn close(&self) {
        self.0.close();
    }

    fn context_lost(&self) -> bool {
        self.0.resumed.get().is_some_and(|(_, lost)| *lost)
    }

    fn move_to(&self, account: String) -> BoxFuture<'_, Result<(), agent::Error>> {
        Box::pin(self.0.move_to(account))
    }

    fn stop_task(&self, task_id: String) -> BoxFuture<'_, Result<(), agent::Error>> {
        Box::pin(self.0.stop_task(task_id))
    }

    fn task_output(&self, task_id: String) -> BoxFuture<'_, Result<TaskOutput, agent::Error>> {
        Box::pin(self.0.task_output(task_id))
    }

    fn processes(&self) -> BoxFuture<'_, Result<Vec<wire::app::SessionProcess>, agent::Error>> {
        Box::pin(async {
            self.0
                .l
                .call(&self.0.rt, to_runtime::client::session_processes(self.0.id.clone()))
                .await
                .map_err(|e| e.agent("OS processes"))
        })
    }

    fn kill_process(&self, kill_token: String) -> BoxFuture<'_, Result<(), agent::Error>> {
        Box::pin(async move {
            self.0
                .l
                .call(
                    &self.0.rt,
                    to_runtime::client::session_process_kill(self.0.id.clone(), kill_token),
                )
                .await
                .map(|_| ())
                .map_err(|e| e.agent("force kill process"))
        })
    }

    fn context(&self) -> BoxFuture<'_, Result<model::SessionContext, agent::Error>> {
        Box::pin(self.0.context())
    }
}
