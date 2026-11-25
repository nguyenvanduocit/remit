//! The core's link to its AI runtime (`remit runtime`), the child process that holds every Claude Code session. The
//! runtime dials the core on the link, comes in with the key this start of the core minted (`Key`), and the two speak
//! `wire::runtime` both ways on that one session (ADR-0058, ADR-0072 decision 3). `Launcher` is the core's one session
//! port, `agent::Launcher`, over it, so the orchestrator starts, prompts, steers and closes a session there as it would
//! here; `Accounts` and `Transcripts` reach the accounts' config dirs and Claude Code's transcripts, which live with the
//! runtime; `supervise` keeps the runtime's process running.
//!
//! The runtime is called through `wire::runtime`'s `client` stubs on a `Link`, the port both ends of the contract use:
//! `link_quic::Link` in the binary, `memory::Link` in tests.

mod accounts;
mod convert;
mod launcher;
pub mod memory;
mod port;
mod quic;
mod serve;
mod session;
mod supervise;
mod transcripts;

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use subtle::ConstantTimeEq;

pub use accounts::{Accounts, Login};
pub use launcher::{Launcher, Options, State};
pub use port::{Link, Peer, dropped};
pub use supervise::{BoxError, Process};
pub use transcripts::{Transcript, Transcripts};

/// Starts the refusal of a hello while another runtime is connected: a dead session the link has not yet timed out. It
/// passes, so the runtime says hello again.
pub const BUSY_REFUSAL: &str = "another AI runtime is connected";

/// The clock both ends read; tests give a fixed one.
pub type Clock = Arc<dyn Fn() -> model::Time + Send + Sync>;

/// What a runtime presents in its hello to come in: minted for one start of the core and handed to the runtime it
/// starts, so only that runtime connects. It prints as nothing of itself.
#[derive(Clone, PartialEq, Eq)]
pub struct Key(String);

impl Key {
    /// Mints a key: 32 random bytes, base64url.
    pub fn new() -> Result<Self, getrandom::Error> {
        let mut b = [0u8; 32];
        getrandom::fill(&mut b)?;
        Ok(Self(URL_SAFE_NO_PAD.encode(b)))
    }

    /// The key as it is handed to the runtime.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Says `credential` is this key. The comparison takes the same time however much of it matches; one of another
    /// length is refused at once, which tells its length and nothing of the key's bytes.
    pub fn admits(&self, credential: &str) -> bool {
        !self.0.is_empty() && bool::from(credential.as_bytes().ct_eq(self.0.as_bytes()))
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Key(..)")
    }
}

/// Why an account call or a transcript read has no answer.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the AI runtime: {0}")]
    Link(link::Failure),
    /// The runtime ended before the call could be made on it.
    #[error("the AI runtime {0} is gone: it restarted")]
    Gone(String),
    /// The runtime answered that it cannot do it, and why.
    #[error("{0}")]
    Refused(String),
    #[error("the transcript {0} is not written yet")]
    NoTranscript(String),
    #[error("the transcript {path}: {why}")]
    Transcript { path: String, why: String },
    /// The AI runtime cannot be run: the first one could not be started, or exited before its hello.
    #[error("{0}")]
    Runtime(String),
}

/// The id of a call the runtime acts on once (session, prompt, steer, request and login ids).
pub(crate) fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// A lock no panic poisons: every section under it leaves the state whole.
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}
