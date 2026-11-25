//! Decides WHERE a program in a jail may connect, and relays the connections that pass (ADR-0018, forms T1 and T2, "Where
//! the executor runs").
//!
//! A jail has no network but one unix socket. The `Gate` listens on that socket as an HTTP forward proxy (CONNECT for
//! HTTPS, an absolute URI for plain HTTP) and connects a program only to the host:port pairs the caller listed when it
//! made the Gate. Everything else is refused with 403 and counted, so the answer to the owner can say what the program
//! tried and was not let do.
//!
//! The Gate carries no credential and reads no request: a CONNECT tunnel is bytes it cannot see. What the Gate stops is a
//! connection to anywhere else: another host, a port that was not listed, and the machine's own network. A name on the
//! allowlist is resolved once and the address checked before it is dialled, so a name that points at 127.0.0.1, a
//! private range or a link-local address (the cloud metadata service) is refused; only an address the caller wrote as a
//! literal on the allowlist skips that check, because the caller named it.
//!
//! Every tunnel has a deadline and a byte cap, and the Gate a cap on tunnels open at once. It follows no redirect.
//!
//! The `Injector` (inject) is the Gate of form T1: the same socket, deadline, caps and refusals, with the connections of
//! one run as its destinations (an `Upstream` each) and no tunnel to any. It ends the program's TLS itself with a
//! certificate made in the run for the connection the program named (inject_cert), reads each request, admits what that
//! connection's pure rules of inject_rules admit (reads only, or writes where the caller grants them), adds that
//! connection's credential headers and answers from the real upstream over a connection of its own (inject_exchange), with
//! the credential scrubbed from the answer. The run's request count and byte budget are shared by its connections.

mod denied;
mod egress;
mod forward;
mod inject;
mod inject_cert;
mod inject_exchange;
mod inject_rules;
mod policy;
mod tunnel;

pub use denied::Denial;
pub use egress::{Config, Error, Gate, Resolver, SystemResolver};
pub use inject::{InjectConfig, Injector, Scrub, Sign, Upstream};
pub use inject_rules::RulesError;
pub use policy::{PolicyError, TargetError};
