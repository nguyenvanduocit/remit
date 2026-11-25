//! How a channel answers from what the orchestrator returned: a record as it is (the contract carries `model`'s and
//! `wire`'s types, ADR-0107), a failure as its kind's code. Every channel on `/link` answers this way (the web app's
//! `Api`, a mentor's device's).

use link::{ErrorCode, Failure};
use wire::Ack;

/// A failure, logged once here, as the error a channel is answered with: its kind as the code, its words as the message,
/// its detail (`model::FailureDetail`) as it is.
pub fn failed(method: &str, err: &orch::Error) -> Failure {
    tracing::warn!(method, %err, "rpc error");
    let code = match err.kind() {
        orch::ErrorKind::NotFound => ErrorCode::NotFound,
        orch::ErrorKind::Ended => ErrorCode::Ended,
        orch::ErrorKind::Unavailable => ErrorCode::Unavailable,
        orch::ErrorKind::Conflict => ErrorCode::Conflict,
        orch::ErrorKind::Failed => ErrorCode::Failed,
    };
    let failure = Failure::new(code, err.to_string());
    match err.detail() {
        Some(detail) => failure.with_detail(&detail),
        None => failure,
    }
}

/// What the orchestrator returned, already the contract's type (a record `model` defines, ADR-0107).
pub fn returned<T>(method: &str, got: orch::Result<T>) -> Result<T, Failure> {
    got.map_err(|e| failed(method, &e))
}

/// The answer to a call with nothing to return.
pub fn done<D>(method: &str, got: orch::Result<D>) -> Result<Ack, Failure> {
    got.map(|_| Ack {}).map_err(|e| failed(method, &e))
}
