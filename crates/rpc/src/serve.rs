//! The web app's way in: a session on `/link` whose hello presents the owner's credential (the owner key, or a Google
//! sign-in this server minted, ADR-0105) or a mentor's worker key (ADR-0089), and runs the bundle this server serves, is
//! granted `wire::app`. The owner's calls are answered by `Api` and he is pushed
//! every change; a mentor's by `MentorApi`, which refuses all but theirs, and they are pushed what reaches them
//! (`Broadcast`). Each is pushed from the moment it came in, and loads its snapshot after it is welcomed, so what it
//! reads is never older than a change it is not pushed.

use std::sync::Arc;
use std::time::Duration;

use link::{Hello, Refusal, RefusalCode};
use orch::Orchestrator;
use wire::app;

use crate::api::Api;
use crate::broadcast::{Broadcast, Subscription};
use crate::mentor_api::MentorApi;
use crate::session;

/// How long a worker key's check may take before the page is told to try again.
const KEY_CHECK: Duration = Duration::from_secs(10);

/// The channel the owner acts through here, as the actors a session signs in say.
pub const CHANNEL: &str = "web";

/// What a refused hello is told when its credential does not let it in: final, the shell shows the way in again.
pub const BAD_TOKEN: &str = "bad token";

/// What a signed-in client running another bundle than the one this server serves is told: the calls and pushes it
/// knows may be named otherwise than the server's now, so it loads itself again rather than come in.
pub const NEW_BUNDLE: &str = "new bundle";

/// Who a session signed in as.
#[derive(Debug, Clone)]
pub enum Principal {
    /// The owner, as the gate names him.
    Owner(String),
    /// A mentor, with a code the owner had made for them.
    Mentor(model::WorkerKey),
}

/// Says who a credential is (the owner key, or the owner's Google sign-in), or why it may not come in.
pub trait Verifier: Send + Sync {
    fn verify(&self, credential: &str) -> Result<String, String>;
}

impl Verifier for auth::OwnerGate {
    fn verify(&self, credential: &str) -> Result<String, String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
            .unwrap_or(0);
        auth::OwnerGate::verify(self, credential, now).map_err(|e| e.to_string())
    }
}

/// Serves `wire::app` to every session the gate lets in.
#[derive(Clone)]
pub struct Server {
    o: Arc<Orchestrator>,
    broadcast: Arc<Broadcast>,
    gate: Option<Arc<dyn Verifier>>,
    bundle: Option<String>,
}

impl Server {
    /// Serves the orchestrator to every session the gate lets in, while this server runs `bundle`, the web bundle it
    /// serves (none: it serves none, and holds no client to one). With no gate nobody comes in: not having set sign-in up
    /// is never an open door. `broadcast` is the `Notifier` the orchestrator was made with.
    pub fn new(
        o: Arc<Orchestrator>,
        broadcast: Arc<Broadcast>,
        gate: Option<Arc<dyn Verifier>>,
        bundle: Option<String>,
    ) -> Self {
        Self {
            o,
            broadcast,
            gate,
            bundle,
        }
    }

    /// Who the hello signed in as, or why it may not come in: its credential first (the owner key, else a worker key
    /// used from addr), then its bundle.
    pub async fn admit(&self, hello: &Hello, addr: &str) -> Result<Principal, Refusal> {
        let refused = |code, reason: &str| Refusal {
            code,
            reason: reason.to_owned(),
        };
        if hello.credential.is_empty() {
            return Err(refused(RefusalCode::BadCredential, BAD_TOKEN));
        }
        let owner = self.gate.as_deref().map(|gate| gate.verify(&hello.credential));
        let who = match owner {
            Some(Ok(who)) => Principal::Owner(who),
            Some(Err(_)) | None => self.worker(&hello.credential, addr).await?,
        };
        if stale(hello.bundle.as_deref(), self.bundle.as_deref()) {
            tracing::info!(?who, bundle = ?hello.bundle, serving = ?self.bundle, "a client on another bundle is told to load itself again");
            return Err(refused(RefusalCode::NewBundle, NEW_BUNDLE));
        }
        Ok(who)
    }

    /// The mentor whose worker key credential is, used from addr, or why it may not come in.
    async fn worker(&self, credential: &str, addr: &str) -> Result<Principal, Refusal> {
        let refused = |reason: &str| Refusal {
            code: RefusalCode::BadCredential,
            reason: reason.to_owned(),
        };
        match tokio::time::timeout(KEY_CHECK, self.o.keys.authorize_worker_key(credential, addr)).await {
            Ok(Ok(Some(k))) => Ok(Principal::Mentor(k)),
            Ok(Ok(None)) => {
                tracing::warn!(addr, "sign-in refused");
                Err(refused(BAD_TOKEN))
            }
            Ok(Err(err)) => {
                tracing::error!(addr, %err, "worker key check");
                Err(refused("the server could not check this key: try again"))
            }
            Err(_) => {
                tracing::error!(addr, "worker key check: no answer in time");
                Err(refused("the server could not check this key: try again"))
            }
        }
    }

    /// Serves one session whose `hello` the caller took off its events: refused, or admitted to `wire::app` and answered
    /// as who signed in until it closes. It subscribes before it answers anything, so a snapshot the client asks for is
    /// read after every change it will not be pushed. A worker key records the host it was used from, the remote
    /// address without its port.
    pub async fn serve(&self, hello: Hello, peer: link_quic::Peer) {
        let addr = peer.remote.ip().to_string();
        let who = match self.admit(&hello, &addr).await {
            Ok(who) => who,
            Err(refusal) => {
                peer.link.refuse(refusal);
                return;
            }
        };
        let node = hello.node.id.clone();
        match who {
            Principal::Owner(who) => {
                let sub = self.broadcast.subscribe();
                let api = Arc::new(Api::new(
                    Arc::clone(&self.o),
                    model::by_owner(&who, CHANNEL),
                    sub.showing(),
                ));
                tracing::info!(node = %node, who, sessions = self.broadcast.sessions(), "a client connected");
                let reason = run(peer, sub, api).await;
                tracing::info!(node = %node, who, reason, sessions = self.broadcast.sessions(), "a client left");
            }
            Principal::Mentor(key) => {
                let sub = self.broadcast.subscribe_mentor(key.clone());
                match self.o.mentor_helps(&key.id).await {
                    Ok(helps) => sub.seed(&helps),
                    Err(err) => {
                        tracing::error!(mentor = key.mentor, %err, "a mentor's helps not read");
                        peer.link.close();
                        return;
                    }
                }
                // The mentor is in the app while this is held.
                let online = self.o.crew.device_online(&key.mentor, &key.id);
                let api = Arc::new(MentorApi::new(Arc::clone(&self.o), key.clone(), sub.showing()));
                tracing::info!(node = %node, mentor = key.mentor, key = key.id, addr, "a mentor connected");
                let reason = run(peer, sub, api).await;
                drop(online);
                tracing::info!(node = %node, mentor = key.mentor, key = key.id, reason, "a mentor left");
            }
        }
    }
}

/// Admits the session to `wire::app` and answers it with handler, pushing it what sub is queued, until it ends; why.
async fn run<H: app::Handler + Send + 'static>(peer: link_quic::Peer, sub: Subscription, handler: Arc<H>) -> String {
    let link_quic::Peer { link, mut events, .. } = peer;
    link.admit(vec![app::CAPABILITY.to_owned()]);
    let behind = sub.behind.clone();
    session::run(
        &link,
        &mut events,
        || sub.next(),
        &behind,
        |push| push.home().map(str::to_owned),
        |method, body| {
            let handler = Arc::clone(&handler);
            async move { app::serve(handler.as_ref(), &method, &body).await }
        },
    )
    .await
}

/// Whether a page runs another web bundle (`theirs`) than the one this server serves (`ours`). A page that names none (the dev server's, which runs the source) and a server that serves none (a test) are
/// held to no bundle.
fn stale(theirs: Option<&str>, ours: Option<&str>) -> bool {
    match (theirs, ours) {
        (Some(theirs), Some(ours)) => !theirs.is_empty() && !ours.is_empty() && theirs != ours,
        _ => false,
    }
}
