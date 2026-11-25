//! The native client of a Remit server (ADR-0072): a binary over `link`, `wire` and `link-quic`. `dial` opens a session
//! on a server's link and `call` types a call by its contract; on them, `app` is the owner's contract from a shell.
//! `remit update` and `remit doctor` come in through `dial` too.

pub mod app;
pub mod call;
pub mod dial;
