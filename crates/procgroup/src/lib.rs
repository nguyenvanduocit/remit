//! Starts child processes: as process groups, so ending a command ends everything it started, or detached, so ending
//! the server does not end it.

mod detach;
mod procgroup;

pub use detach::{detach, running};
pub use procgroup::{Child, Command, Error, WAIT_DELAY, command};
