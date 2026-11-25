//! The one place the core knows which runtime is connected: the runtime each hello names, its connection, the sessions
//! and sign-ins it holds, and the calls made to it, sent again on its next connection when the one they went out on
//! dropped.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use agent::{BoxFuture, Handler, Spec};
use link::{Failure, Outgoing};
use serde::de::DeserializeOwned;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use wire::runtime::to_runtime;

use crate::accounts::LoginState;
use crate::convert::{Ending, session_spec};
use crate::port::{Connection, Link, dropped};
use crate::session::{Handle, Session};
use crate::supervise::Supervised;
use crate::{Clock, Error, Key, lock, new_id};

/// How long a session's exit may take to arrive after `close` asks for it before the session is given up on.
const DEFAULT_KILL_WAIT: Duration = Duration::from_secs(10);

/// How the launcher runs.
pub struct Options {
    /// What the runtime this start of the core runs presents to come in.
    pub key: Key,
    /// The clock `State` reads.
    pub now: Clock,
    /// How long a session's exit may take to arrive after `close` asks for it: past it the supervised runtime is killed
    /// whole, which ends the session with it. Ten seconds when none.
    pub kill_wait: Option<Duration>,
}

/// What the core knows of its runtime, for /healthz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub connected: bool,
    pub runtime_id: String,
    pub revision: String,
    /// Why no runtime is connected; empty while one is.
    pub reason: String,
    /// When `connected` last changed.
    pub since: model::Time,
    pub restarts: u64,
}

/// Starts sessions in the runtime that is connected. Cloning it is another handle on the same launcher.
pub struct Launcher<L> {
    pub(crate) inner: Arc<Inner<L>>,
}

impl<L> Clone for Launcher<L> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

pub(crate) struct Inner<L> {
    pub(crate) key: Key,
    pub(crate) now: Clock,
    pub(crate) kill_wait: Duration,
    shared: Mutex<Shared<L>>,
    /// Sent on every change of the runtime, of its connection or of the process the launcher supervises.
    changed: watch::Sender<()>,
}

pub(crate) struct Shared<L> {
    /// The runtime the sessions run in; none before the first hello and after it exited.
    pub(crate) rt: Option<Arc<Runtime<L>>>,
    pub(crate) state: State,
    /// How many runtimes have said hello: `supervise` sees its process connect by it.
    pub(crate) hellos: u64,
    pub(crate) next_conn: u64,
    pub(crate) next_proc: u64,
    /// The process `supervise` runs; none when none does.
    pub(crate) proc: Option<Supervised<L>>,
}

/// One run of `remit runtime`, known by the id it says hello with. It outlives a dropped session: the same runtime
/// dialling back finds its sessions where they were. It ends when it exits or another runtime takes its place.
pub(crate) struct Runtime<L> {
    pub(crate) id: String,
    /// Cancelled once the runtime has ended.
    pub(crate) gone: CancellationToken,
    held: Mutex<Held<L>>,
}

pub(crate) struct Held<L> {
    /// The session calls go out on; none while the runtime is disconnected.
    pub(crate) conn: Option<Connection<L>>,
    /// By the core's session id.
    pub(crate) sessions: HashMap<String, Arc<Session<L>>>,
    /// The sign-ins the core started in it, by login id.
    pub(crate) logins: HashMap<String, Arc<LoginState<L>>>,
}

impl<L> Runtime<L> {
    pub(crate) fn new(id: String) -> Self {
        Self {
            id,
            gone: CancellationToken::new(),
            held: Mutex::new(Held {
                conn: None,
                sessions: HashMap::new(),
                logins: HashMap::new(),
            }),
        }
    }

    pub(crate) fn held(&self) -> MutexGuard<'_, Held<L>> {
        lock(&self.held)
    }
}

/// Why a call to the runtime has no answer.
pub(crate) enum CallError {
    Link(Failure),
    /// The runtime ended before the call could be made on it: it restarted.
    Gone(String),
}

impl CallError {
    /// The error a session's call fails with: a runtime that ended is the host's restart.
    pub(crate) fn agent(self, what: &str) -> agent::Error {
        match self {
            CallError::Gone(_) => agent::Error::HostRestarted,
            CallError::Link(f) => agent::Error::TurnFailed(format!("{what}: the AI runtime answered {f}")),
        }
    }
}

impl From<CallError> for Error {
    fn from(e: CallError) -> Self {
        match e {
            CallError::Link(f) => Error::Link(f),
            CallError::Gone(id) => Error::Gone(id),
        }
    }
}

impl<L: Link> Launcher<L> {
    /// A launcher with no runtime yet: `serve` lets one in.
    pub fn new(o: Options) -> Self {
        let state = State {
            connected: false,
            runtime_id: String::new(),
            revision: String::new(),
            reason: "no AI runtime has connected yet".to_owned(),
            since: (o.now)(),
            restarts: 0,
        };
        Self {
            inner: Arc::new(Inner {
                key: o.key,
                now: o.now,
                kill_wait: o.kill_wait.unwrap_or(DEFAULT_KILL_WAIT),
                shared: Mutex::new(Shared {
                    rt: None,
                    state,
                    hellos: 0,
                    next_conn: 0,
                    next_proc: 0,
                    proc: None,
                }),
                changed: watch::channel(()).0,
            }),
        }
    }

    pub(crate) fn shared(&self) -> MutexGuard<'_, Shared<L>> {
        lock(&self.inner.shared)
    }

    pub(crate) fn now(&self) -> model::Time {
        (self.inner.now)()
    }

    /// Wakes every waiter on a change.
    pub(crate) fn bump(&self) {
        self.inner.changed.send_replace(());
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<()> {
        self.inner.changed.subscribe()
    }

    /// Whether a runtime is connected, which, and since when.
    pub fn state(&self) -> State {
        self.shared().state.clone()
    }

    /// Returns once a runtime has said hello and is connected; the caller bounds the wait.
    pub async fn ready(&self) {
        self.connected().await;
    }

    /// The runtime that is connected, once one is.
    pub(crate) async fn connected(&self) -> Arc<Runtime<L>> {
        let mut changed = self.subscribe();
        loop {
            if let Some(rt) = self.shared().rt.clone()
                && rt.held().conn.is_some()
            {
                return rt;
            }
            // The sender lives as long as this launcher: the wait ends with a change.
            changed.changed().await.ok();
        }
    }

    /// Records why a runtime that said hello was turned away, while none is connected: /healthz says it.
    pub(crate) fn refused(&self, why: &str) {
        let now = self.now();
        let mut shared = self.shared();
        if !shared.state.connected {
            shared.state.reason = why.to_owned();
            shared.state.since = now;
        }
    }

    /// Makes one call to the connected runtime, waiting for one.
    pub(crate) async fn on_runtime<A>(&self, out: Outgoing<to_runtime::Call, A>) -> Result<A, CallError>
    where
        A: DeserializeOwned + Send + 'static,
    {
        let rt = self.connected().await;
        self.call(&rt, out).await
    }

    /// Makes one call to `rt`, sent again on the runtime's next connection when the one it went out on dropped. It fails
    /// `Gone` once `rt` has ended.
    pub(crate) async fn call<A>(&self, rt: &Runtime<L>, out: Outgoing<to_runtime::Call, A>) -> Result<A, CallError>
    where
        A: DeserializeOwned + Send + 'static,
    {
        loop {
            let conn = self.connection(rt).await?;
            match conn.link.call(out.clone()).await {
                Err(f) if dropped(&f) => self.disconnected(rt, conn.n, &f.message),
                answer => return answer.map_err(CallError::Link),
            }
        }
    }

    /// `rt`'s connection, once it has one.
    async fn connection(&self, rt: &Runtime<L>) -> Result<Connection<L>, CallError> {
        let mut changed = self.subscribe();
        loop {
            if rt.gone.is_cancelled() {
                return Err(CallError::Gone(rt.id.clone()));
            }
            if let Some(conn) = rt.held().conn.clone() {
                return Ok(conn);
            }
            tokio::select! {
                _ = changed.changed() => {}
                () = rt.gone.cancelled() => {}
            }
        }
    }

    /// Records that connection `n` of `rt` dropped; a later connection of `rt` is not touched.
    pub(crate) fn disconnected(&self, rt: &Runtime<L>, n: u64, reason: &str) {
        let now = self.now();
        let mut shared = self.shared();
        {
            let mut held = rt.held();
            if held.conn.as_ref().is_none_or(|c| c.n != n) {
                return;
            }
            held.conn = None;
        }
        if shared.rt.as_ref().is_some_and(|cur| std::ptr::eq(cur.as_ref(), rt)) {
            shared.state.connected = false;
            shared.state.reason = format!("the AI runtime disconnected: {reason}");
            shared.state.since = now;
        }
        drop(shared);
        self.bump();
    }

    /// Ends `rt` and every session and sign-in it held, for `why`.
    pub(crate) fn end(&self, rt: &Runtime<L>, why: &Ending) {
        let (sessions, logins) = {
            let _shared = self.shared();
            if rt.gone.is_cancelled() {
                return;
            }
            rt.gone.cancel();
            let held = rt.held();
            let sessions: Vec<_> = held.sessions.values().filter(|s| s.is_opened()).cloned().collect();
            let logins: Vec<_> = held.logins.values().cloned().collect();
            (sessions, logins)
        };
        self.bump();
        for s in sessions {
            s.finish(why.clone());
        }
        let ended = match why {
            Ending::Restarted(why) => why.clone(),
            Ending::Exited(_) | Ending::Unheld => format!("the AI runtime {} ended", rt.id),
        };
        for login in logins {
            login.end(Some(ended.clone()));
        }
    }

    /// Makes `s` one of `rt`'s sessions before it is opened, so the entries the runtime sends while it opens it, its
    /// opened entry last, find it.
    pub(crate) fn hold(&self, rt: &Runtime<L>, s: &Arc<Session<L>>) -> Result<(), agent::Error> {
        let _shared = self.shared();
        if rt.gone.is_cancelled() {
            return Err(agent::Error::HostRestarted);
        }
        rt.held().sessions.insert(s.id.clone(), Arc::clone(s));
        Ok(())
    }

    /// Marks `s` opened: from now on the runtime's end ends it. A session whose exit arrived before its open's answer did
    /// not open.
    pub(crate) fn open(&self, rt: &Runtime<L>, s: &Session<L>) -> Result<(), agent::Error> {
        let _shared = self.shared();
        if rt.gone.is_cancelled() {
            return Err(agent::Error::HostRestarted);
        }
        s.mark_opened()
    }

    pub(crate) fn drop_session(&self, rt: &Runtime<L>, s: &Session<L>) {
        let mut held = rt.held();
        if held
            .sessions
            .get(&s.id)
            .is_some_and(|held| std::ptr::eq(held.as_ref(), s))
        {
            held.sessions.remove(&s.id);
        }
    }

    /// Opens a session in the connected runtime, waiting for one. The runtime takes the open at once and says how it
    /// ended in the session's opened entry; the caller bounds the whole open, Claude's start included. A start given up
    /// (its future dropped, or failed on the link) kills what the runtime may be opening: nobody would close it.
    async fn start_session(&self, spec: Spec, h: Arc<dyn Handler>) -> Result<Box<dyn agent::Session>, agent::Error> {
        let rt = self.connected().await;
        let s = Session::new(self.clone(), Arc::clone(&rt), new_id(), h);
        self.hold(&rt, &s)?;
        let mut abandon = Abandon(Some(Arc::clone(&s)));
        let opened = match self
            .call(&rt, to_runtime::client::session_open(session_spec(&s.id, &spec)))
            .await
        {
            Ok(_) => s.until_opened().await,
            Err(e) => Err(e.agent("open a session")),
        }?;
        abandon.0 = None;
        if let Some(failure) = opened.failure {
            self.drop_session(&rt, &s);
            return Err(agent::Error::Failure(failure));
        }
        if !opened.refused.is_empty() {
            self.drop_session(&rt, &s);
            return Err(agent::Error::TurnFailed(format!(
                "the AI runtime did not open the session: {}",
                opened.refused
            )));
        }
        s.resumed(opened.resume_id, opened.context_lost);
        self.open(&rt, &s)?;
        Ok(Box::new(Handle(s)))
    }
}

/// Kills the session of a start that gave up, unless the start got its answer.
struct Abandon<L: Link>(Option<Arc<Session<L>>>);

impl<L: Link> Drop for Abandon<L> {
    fn drop(&mut self) {
        if let Some(s) = self.0.take() {
            s.abandon();
        }
    }
}

impl<L: Link> agent::Launcher for Launcher<L> {
    fn start(&self, spec: Spec, h: Arc<dyn Handler>) -> BoxFuture<'_, Result<Box<dyn agent::Session>, agent::Error>> {
        Box::pin(self.start_session(spec, h))
    }
}
