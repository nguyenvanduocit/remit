//! Starts child processes: as process groups, so ending a command ends everything it started, or detached, so ending
//! the server does not end it. `Program` is the one runner every child program but a session goes through (ADR-0138).

mod detach;
mod procgroup;
mod program;

pub use detach::{detach, running};
pub use procgroup::{Child, Command, Error, WAIT_DELAY, command};
pub use program::{Limits, Program, Ran};
