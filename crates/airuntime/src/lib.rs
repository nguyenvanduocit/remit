//! The AI runtime (`remit runtime`): the process that holds every Claude Code session of a Remit core, the accounts'
//! config dirs with their logins, and the transcripts Claude Code writes. It dials the core that started it on the link,
//! comes in with the key that core minted, and speaks `wire::runtime` both ways on that one session (ADR-0058, ADR-0072
//! decision 3): the core opens, prompts, steers, cancels and kills sessions here, and what a session does goes back as
//! numbered entries, sent again after a dropped session until the core has them.
//!
//! How it dials is a port (`Dial`): `link-quic` in the binary, `runtimelink::memory` in tests.

mod accounts;
mod calls;
mod convert;
mod runtime;
mod session;
mod transcripts;

use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use runtimelink::{BoxError, Clock, Link, Peer};
use tokio_util::sync::CancellationToken;

pub use accounts::{Accounts, Login};
pub use transcripts::TranscriptDir;

/// How the runtime reaches the core: one session of the link per call, its hello said as given. The session that
/// drops is dialled again.
pub trait Dial: Send + Sync {
    type Link: Link;
    fn dial(&self, hello: link::Hello) -> impl Future<Output = Result<Peer<Self::Link>, BoxError>> + Send;
}

/// What a runtime runs with.
pub struct Config {
    /// The key this start of the core minted.
    pub key: String,
    /// The build this runtime runs.
    pub revision: String,
    /// `agent::ClaudeLauncher` in production.
    pub launcher: Arc<dyn agent::Launcher>,
    /// The accounts' config dirs and logins; none holds none.
    pub accounts: Option<Arc<dyn Accounts>>,
    /// Where Claude Code writes the sessions' transcripts; none serves none.
    pub transcripts: Option<TranscriptDir>,
    pub now: Clock,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The core that started this runtime is gone: its key is refused. A core that started since has a key of its own
    /// and a runtime of its own.
    #[error("the core that started this runtime is gone: its key is refused ({0})")]
    CoreGone(String),
    #[error("{0}")]
    Refused(String),
}

/// Serves the core through `dial` until `life` is cancelled or the core refuses this runtime, and returns once every
/// session it holds is closed.
pub async fn run<D: Dial>(life: CancellationToken, dial: D, cfg: Config) -> Result<(), Error> {
    let r = runtime::Runtime::<D::Link>::new(cfg, life.child_token());
    let served = r.serve(&dial).await;
    r.life.cancel();
    r.close_all().await;
    served
}

/// The id of a request the runtime asks the core once.
pub(crate) fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// A lock no panic poisons: every section under it leaves the state whole.
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}
