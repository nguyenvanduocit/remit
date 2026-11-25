//! Runs a program where it can touch nothing of Remit (ADR-0018, "Where the executor runs").
//!
//! [`run`] starts a program whose files are its work directory and the read-only system paths it was given, whose
//! environment is the one it was given, and whose only way out is a unix socket it was given. The jail relays a port of
//! 127.0.0.1 to that socket and points the program's proxy variables at it, so ordinary tooling reaches the socket with no
//! knowledge of unix sockets. What listens on the other end decides where the program may connect (`egress`: the gate);
//! the jail's part is to leave the program nothing else.
//!
//! [`hidden_with`] is the lighter form for an agent session: the session keeps the machine and its network, the paths that
//! hold Remit's secrets cannot be read, and the paths that must not change under it stay readable but read-only.
//!
//! Each operating system has an adapter of its own behind this one API; neither needs privilege:
//!
//!   - Linux (`*_linux.rs`): new user, mount, network and pid namespaces. A hidden path reads as empty.
//!   - macOS (`*_darwin.rs`): Seatbelt, applied by /usr/bin/sandbox-exec (ADR-0021). A hidden path refuses to be read. The
//!     memory limit is not applied, as macOS refuses the rlimits that would hold it.
//!
//! A program starts through a helper, which is this same binary started again; the binary's `main` calls [`init`] first,
//! as the tests' `main` does. The helper gets its plan in its arguments (the plan holds no secret: the environment, which
//! may hold a credential, is the helper's own environment and never written to argv or disk) and reports on a unix socket
//! in a directory only this user can enter, whose path is in its arguments.
//!
//! What a jail does not do, on purpose: it bounds no process count and no disk; a hard link to a hidden file made
//! elsewhere is not hidden; a program gains nothing by running a setuid program.

mod lookpath;
mod relay;
mod run;
mod spawn;
mod spec;
mod status;
mod wrap;

#[cfg(target_os = "macos")]
mod init_darwin;
#[cfg(target_os = "macos")]
mod plan_darwin;
#[cfg(target_os = "macos")]
mod sandbox_darwin;
#[cfg(target_os = "macos")]
mod sweep_darwin;

#[cfg(target_os = "linux")]
mod init_linux;
#[cfg(target_os = "linux")]
mod mounts_linux;
#[cfg(target_os = "linux")]
mod namespaces_linux;
#[cfg(target_os = "linux")]
mod plan_linux;
#[cfg(target_os = "linux")]
mod supervise_linux;

#[cfg(target_os = "macos")]
use init_darwin as os_init;
#[cfg(target_os = "linux")]
use init_linux as os_init;
#[cfg(target_os = "macos")]
use plan_darwin as os_plan;
#[cfg(target_os = "linux")]
use plan_linux as os_plan;

pub use os_plan::{ca_paths, system_paths};
pub use run::run;
pub use spawn::{Jailed, spawn};
pub use spec::{Error, RunResult, Spec};
pub use wrap::{Command, Mask, Wrap, hidden_with};

/// The first call of a binary that uses this crate, in `main` and in a test binary's `main`, before any thread or runtime
/// starts. In a process the parent started as the helper it runs the program under the plan and exits with the
/// program's status; in any other process it returns at once.
pub fn init() {
    os_init::init();
}

/// argv\[1\] (macOS) or argv\[0\] (Linux) of the helper, the binary started again to set a jail up. A process with this
/// name is the helper whatever else it was given.
const HELPER_NAME: &str = "remit-jail-init";

/// The exit status of a helper that could not run the program; why is on the status socket.
const HELPER_FAILED: i32 = 125;
