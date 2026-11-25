//! One run of the runtime: one id for its whole life, the sessions and sign-ins it holds, and the session of the link it
//! serves the core on, dialled again when it drops.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use link::{Event, RefusalCode};
use runtimelink::{BUSY_REFUSAL, Link, Peer, dropped};
use time::format_description::well_known::Rfc3339;
use tokio::sync::{OnceCell, mpsc, watch};
use tokio_util::sync::CancellationToken;
use wire::runtime::{Hello, RUNTIME_CONTRACT, SessionAck, to_core, to_runtime};

use crate::accounts::Login;
use crate::calls::Calls;
use crate::session::Session;
use crate::{Config, Dial, Error, lock};

/// How long a call that failed on a session still up waits before it is made again, and the first wait of a hello
/// said again; the wait doubles to `MAX_BACKOFF`.
const RETRY_AFTER: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

pub(crate) struct Runtime<L> {
    pub(crate) cfg: Config,
    pub(crate) id: String,
    /// Cancelled when `run` returns.
    pub(crate) life: CancellationToken,
    state: Mutex<State<L>>,
    /// Sent when the connection changes.
    changed: watch::Sender<()>,
}

struct State<L> {
    /// The session the core took the hello on; none while none is up.
    conn: Option<Connection<L>>,
    next_conn: u64,
    sessions: HashMap<String, Arc<Session<L>>>,
    /// Each sign-in by its id, from the first time it is asked: a replay waits for the first and takes its login.
    logins: HashMap<String, Arc<OnceCell<Arc<dyn Login>>>>,
    /// Why the core turned this runtime away.
    refused: Option<String>,
}

/// One session of the link to the core, told apart from the ones before and after it.
#[derive(Clone)]
pub(crate) struct Connection<L> {
    n: u64,
    pub(crate) link: L,
}

impl<L: Link> Runtime<L> {
    pub(crate) fn new(cfg: Config, life: CancellationToken) -> Arc<Self> {
        Arc::new(Self {
            cfg,
            id: uuid::Uuid::new_v4().simple().to_string(),
            life,
            state: Mutex::new(State {
                conn: None,
                next_conn: 0,
                sessions: HashMap::new(),
                logins: HashMap::new(),
                refused: None,
            }),
            changed: watch::channel(()).0,
        })
    }

    fn state(&self) -> MutexGuard<'_, State<L>> {
        lock(&self.state)
    }

    pub(crate) fn now(&self) -> String {
        let at = (self.cfg.now)().0.to_offset(time::UtcOffset::UTC);
        at.format(&Rfc3339).unwrap_or_default()
    }

    /// Keeps a session to the core up, dialling again when the core drops one, until the runtime's life ends or the core
    /// refuses it.
    pub(crate) async fn serve<D: Dial<Link = L>>(self: &Arc<Self>, dial: &D) -> Result<(), Error> {
        loop {
            let hello = link::Hello {
                node: link::Node {
                    id: self.id.clone(),
                    public_key: String::new(),
                },
                credential: self.cfg.key.clone(),
                capabilities: vec![to_core::CAPABILITY.to_owned()],
                offers: vec![to_runtime::CAPABILITY.to_owned()],
                bundle: None,
            };
            let dialled = tokio::select! {
                dialled = dial.dial(hello) => dialled,
                () = self.life.cancelled() => return Ok(()),
            };
            let Peer { link, mut events } = match dialled {
                Ok(peer) => peer,
                Err(e) => {
                    tracing::warn!(error = %e, "dial the core");
                    if !self.wait(RETRY_AFTER).await {
                        return Ok(());
                    }
                    continue;
                }
            };
            let admitted = tokio::select! {
                admitted = admission(&mut events) => admitted,
                () = self.life.cancelled() => {
                    link.close();
                    return Ok(());
                }
            };
            match admitted {
                Admission::Welcomed => {}
                Admission::BadKey(why) => return Err(Error::CoreGone(why)),
                Admission::Lost(why) => {
                    tracing::warn!(why = %why, "the core did not take the session");
                    if !self.wait(RETRY_AFTER).await {
                        return Ok(());
                    }
                    continue;
                }
            }
            let conn = {
                let mut state = self.state();
                state.next_conn = state.next_conn.wrapping_add(1);
                Connection {
                    n: state.next_conn,
                    link,
                }
            };
            let closed = CancellationToken::new();
            tokio::spawn(Arc::clone(self).answer(conn.clone(), events, closed.clone()));
            tokio::select! {
                () = self.greet(&conn) => {}
                () = closed.cancelled() => {}
                () = self.life.cancelled() => {}
            }
            tokio::select! {
                () = closed.cancelled() => {}
                () = self.life.cancelled() => {}
            }
            self.lost(&conn);
            conn.link.close();
            if let Some(why) = self.state().refused.clone() {
                return Err(Error::Refused(why));
            }
            if self.life.is_cancelled() {
                return Ok(());
            }
        }
    }

    /// Answers the core's calls on `conn` until it closes. An action runs to its end whatever happens to the session
    /// that asked for it.
    async fn answer(
        self: Arc<Self>,
        conn: Connection<L>,
        mut events: mpsc::UnboundedReceiver<Event>,
        closed: CancellationToken,
    ) {
        let calls = Arc::new(Calls { r: Arc::clone(&self) });
        while let Some(event) = events.recv().await {
            match event {
                Event::Call { id, method, body, .. } => {
                    let calls = Arc::clone(&calls);
                    let link = conn.link.clone();
                    tokio::spawn(async move {
                        let answer = to_runtime::serve(calls.as_ref(), &method, &body).await;
                        link.reply(id, answer);
                    });
                }
                Event::Closed(closing) => {
                    tracing::info!(closing = ?closing, "the core's session closed");
                    break;
                }
                _ => {}
            }
        }
        self.lost(&conn);
        closed.cancel();
    }

    /// Says hello on `conn`: the runtime's id and the sessions it holds. The core's answer says what of each it has, so
    /// each sends again what followed; a held session it does not list is killed. A hello that fails on a session still
    /// up, or that a core holding another runtime's session turns away, is said again until the core takes it.
    async fn greet(&self, conn: &Connection<L>) {
        let mut backoff = RETRY_AFTER;
        loop {
            let held = self.held();
            let hello = Hello {
                runtime_id: self.id.clone(),
                revision: self.cfg.revision.clone(),
                contract: RUNTIME_CONTRACT,
                sessions: held.iter().map(|s| s.held_as()).collect(),
            };
            let again = match conn.link.call(to_core::client::runtime_hello(hello)).await {
                Err(f) if dropped(&f) => {
                    tracing::warn!(error = %f, "say hello to the core");
                    return;
                }
                Err(f) => format!("{f}"),
                Ok(reply) if reply.refused.starts_with(BUSY_REFUSAL) => reply.refused,
                Ok(reply) if !reply.refused.is_empty() => {
                    self.state().refused = Some(format!("the core refused this AI runtime: {}", reply.refused));
                    conn.link.close();
                    return;
                }
                Ok(reply) => {
                    self.welcome(conn, &held, &reply.sessions);
                    return;
                }
            };
            tracing::info!(why = %again, after = ?backoff, "say hello to the core again");
            if !self.wait(backoff).await {
                return;
            }
            backoff = backoff.saturating_mul(2).min(MAX_BACKOFF);
        }
    }

    /// Takes the core's answer to a hello on `conn`: each held session it lists goes on after what the core has, the
    /// others are killed, and `conn` is the one calls go out on.
    fn welcome(&self, conn: &Connection<L>, held: &[Arc<Session<L>>], acks: &[SessionAck]) {
        for s in held {
            match acks.iter().find(|a| a.session_id == s.id) {
                Some(a) => s.ack(a.acked),
                None => {
                    tracing::info!(session = %s.id, "the core does not hold a session: it is killed");
                    s.close();
                }
            }
        }
        self.state().conn = Some(conn.clone());
        self.changed.send_replace(());
        tracing::info!(runtime = %self.id, sessions = held.len(), "the core took the hello");
    }

    /// The sessions the runtime holds: opened, and not yet past the core's ack of their exit.
    fn held(&self) -> Vec<Arc<Session<L>>> {
        self.state()
            .sessions
            .values()
            .filter(|s| s.is_open())
            .cloned()
            .collect()
    }

    /// Notes that `conn` dropped: calls wait for the next session's hello.
    fn lost(&self, conn: &Connection<L>) {
        let mut state = self.state();
        if state.conn.as_ref().is_some_and(|c| c.n == conn.n) {
            state.conn = None;
            drop(state);
            self.changed.send_replace(());
        }
    }

    /// The session the core took the hello on, once there is one; none once the runtime ends.
    pub(crate) async fn connection(&self) -> Option<Connection<L>> {
        let mut changed = self.changed.subscribe();
        loop {
            if let Some(conn) = self.state().conn.clone() {
                return Some(conn);
            }
            tokio::select! {
                _ = changed.changed() => {}
                () = self.life.cancelled() => return None,
            }
        }
    }

    /// Waits after a call on `conn` failed: for another session, or `RETRY_AFTER` on one still up. False when the
    /// runtime ends.
    pub(crate) async fn pause(&self, conn: &Connection<L>) -> bool {
        let mut changed = self.changed.subscribe();
        if self.state().conn.as_ref().is_some_and(|c| c.n != conn.n) {
            // Another session is up already.
            return !self.life.is_cancelled();
        }
        tokio::select! {
            _ = changed.changed() => true,
            () = tokio::time::sleep(RETRY_AFTER) => true,
            () = self.life.cancelled() => false,
        }
    }

    /// Waits `d`; false when the runtime ends first.
    async fn wait(&self, d: Duration) -> bool {
        tokio::select! {
            () = tokio::time::sleep(d) => true,
            () = self.life.cancelled() => false,
        }
    }

    pub(crate) fn session(&self, id: &str) -> Option<Arc<Session<L>>> {
        self.state().sessions.get(id).cloned()
    }

    /// The session `id`, made and started once: false when the runtime held it already.
    pub(crate) fn hold(self: &Arc<Self>, id: &str) -> (Arc<Session<L>>, bool) {
        let mut state = self.state();
        if let Some(s) = state.sessions.get(id) {
            return (Arc::clone(s), false);
        }
        let s = Session::new(Arc::clone(self), id.to_owned());
        state.sessions.insert(id.to_owned(), Arc::clone(&s));
        (s, true)
    }

    /// Lets `s` go, unless another session took its id.
    pub(crate) fn forget(&self, s: &Arc<Session<L>>) {
        let mut state = self.state();
        if state.sessions.get(&s.id).is_some_and(|held| Arc::ptr_eq(held, s)) {
            state.sessions.remove(&s.id);
        }
    }

    /// The sign-in `id`, once it has started.
    pub(crate) fn login(&self, id: &str) -> Option<Arc<dyn Login>> {
        self.state().logins.get(id).and_then(|once| once.get().cloned())
    }

    /// The sign-in `id`, started by `start` once however often it is asked, even while the first start runs: a sign-in
    /// started now is told to the core when it ends (`report_login`). A start that fails leaves the id to be asked again.
    pub(crate) async fn sign_in<F>(self: &Arc<Self>, id: &str, start: F) -> Result<Arc<dyn Login>, agent::Error>
    where
        F: Future<Output = Result<Arc<dyn Login>, agent::Error>>,
    {
        let once = Arc::clone(self.state().logins.entry(id.to_owned()).or_default());
        let login = once
            .get_or_try_init(|| async {
                let login = start.await?;
                tokio::spawn(Arc::clone(self).report_login(id.to_owned(), Arc::clone(&login)));
                Ok(login)
            })
            .await;
        if login.is_err() {
            let mut state = self.state();
            if state
                .logins
                .get(id)
                .is_some_and(|held| Arc::ptr_eq(held, &once) && held.get().is_none())
            {
                state.logins.remove(id);
            }
        }
        login.cloned()
    }

    /// Tells the core how the sign-in `login_id` ended, on every session until it acks, then lets the login go.
    pub(crate) async fn report_login(self: Arc<Self>, login_id: String, login: Arc<dyn Login>) {
        tokio::select! {
            () = login.done() => {}
            () = self.life.cancelled() => {
                self.state().logins.remove(&login_id);
                return;
            }
        }
        let why = login.err().unwrap_or_default();
        while let Some(conn) = self.connection().await {
            let ended = to_core::client::account_login_ended(login_id.clone(), why.clone());
            if conn.link.call(ended).await.is_ok() || !self.pause(&conn).await {
                break;
            }
        }
        self.state().logins.remove(&login_id);
    }

    /// Closes every session the runtime holds and ends every sign-in, and returns once each session has exited.
    pub(crate) async fn close_all(&self) {
        let (sessions, logins) = {
            let state = self.state();
            let sessions: Vec<_> = state.sessions.values().cloned().collect();
            let logins: Vec<_> = state.logins.values().filter_map(|once| once.get().cloned()).collect();
            (sessions, logins)
        };
        for login in logins {
            login.cancel();
        }
        let mut closing = tokio::task::JoinSet::new();
        for s in sessions {
            closing.spawn(async move { s.close_and_wait().await });
        }
        while closing.join_next().await.is_some() {}
    }
}

enum Admission {
    Welcomed,
    /// The core turned the runtime's key away.
    BadKey(String),
    Lost(String),
}

/// How the core answered the runtime's hello on the link.
async fn admission(events: &mut mpsc::UnboundedReceiver<Event>) -> Admission {
    while let Some(event) = events.recv().await {
        match event {
            Event::Welcomed(_) => return Admission::Welcomed,
            Event::Refused(r) if r.code == RefusalCode::BadCredential => {
                return Admission::BadKey(r.reason);
            }
            Event::Refused(r) => return Admission::Lost(r.reason),
            Event::Closed(link::Closing::Refused(r)) if r.code == RefusalCode::BadCredential => {
                return Admission::BadKey(r.reason);
            }
            Event::Closed(closing) => return Admission::Lost(format!("{closing:?}")),
            _ => {}
        }
    }
    Admission::Lost("the session ended".to_owned())
}
