//! Decides what a child process inherits of the server's environment, when an agent can steer what that process does: a
//! Claude Code session and its shell, the build, vet and test of a change, a git command in an agent's clone (its hooks
//! and config run code the agent wrote). The server's environment holds the credentials of Remit's sources and of the
//! host it runs on, and a process reads its environment as plainly as its parent does, so such a child gets the names it
//! needs and nothing else (ADR-0018, invariant 1). The list is the one owner of that decision, and `trace` of what a child
//! is told of the work it runs for (ADR-0119).

mod childenv;
mod trace;

pub use childenv::{of, secret_names};
pub use trace::{Trace, trace};
