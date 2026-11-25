use std::ffi::OsStr;
use std::io;
use std::ops::{Deref, DerefMut};
use std::process::{ExitStatus, Output, Stdio};
use std::time::Duration;

use rustix::process::{Pid, Signal, kill_process_group};
use tokio::io::{AsyncRead, AsyncReadExt};

/// How long `Command::output` waits, once the command has exited, for processes it left behind to release its output
/// before the pipes are closed.
pub const WAIT_DELAY: Duration = Duration::from_secs(5);

/// Why a command could not be run to its end.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("procgroup: {0}")]
    Io(#[from] io::Error),
    /// The command exited, but a process it left behind still held its output after `WAIT_DELAY`.
    #[error("procgroup: the command's output was still held {WAIT_DELAY:?} after it exited")]
    WaitDelay,
    /// The shell that detaches a program did not report it started: the program is not one, or its log cannot be
    /// written.
    #[error("procgroup: the program was not detached ({status}): {stderr}")]
    NotDetached { status: ExitStatus, stderr: String },
}

/// A command that leads its own process group: when its `Child` is dropped before the command was reaped (the caller's
/// future dropped, a `tokio::time::timeout` passed) the whole group is killed (sh and the test binary it runs, an agent
/// and the CLI it spawned), not only the direct child. It derefs to `tokio::process::Command` for the environment,
/// directory and stdio.
pub struct Command {
    inner: tokio::process::Command,
}

/// A command for name with args, as its own process group's leader. The command is the caller's: the owner's agent and
/// tool commands, never a request's input.
pub fn command<I, S>(name: impl AsRef<OsStr>, args: I) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    #[expect(
        clippy::disallowed_methods,
        reason = "procgroup is where a child process starts: every other start goes through command"
    )]
    let mut inner = tokio::process::Command::new(name);
    inner.args(args).process_group(0);
    Command { inner }
}

impl Deref for Command {
    type Target = tokio::process::Command;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Command {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Command {
    /// Starts the command; the group ends when the `Child` is dropped before it is reaped.
    pub fn spawn(&mut self) -> Result<Child, Error> {
        Ok(Child {
            inner: self.inner.spawn()?,
        })
    }

    /// Runs the command to its end and collects its standard output and error, as `std::process::Command::output`:
    /// the exit status is the caller's to judge. Dropping the future kills the group.
    pub async fn output(&mut self) -> Result<Output, Error> {
        self.inner
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = self.spawn()?;
        let (stdout, stderr) = (child.inner.stdout.take(), child.inner.stderr.take());
        let reads = async { tokio::join!(read_all(stdout), read_all(stderr)) };
        tokio::pin!(reads);
        enum First {
            Exited(io::Result<ExitStatus>),
            Read((io::Result<Vec<u8>>, io::Result<Vec<u8>>)),
        }
        let first = tokio::select! {
            status = child.inner.wait() => First::Exited(status),
            read = &mut reads => First::Read(read),
        };
        let (status, (stdout, stderr)) = match first {
            First::Exited(status) => (
                status?,
                tokio::time::timeout(WAIT_DELAY, &mut reads)
                    .await
                    .map_err(|_| Error::WaitDelay)?,
            ),
            First::Read(read) => (child.inner.wait().await?, read),
        };
        Ok(Output {
            status,
            stdout: stdout?,
            stderr: stderr?,
        })
    }
}

async fn read_all(pipe: Option<impl AsyncRead + Unpin>) -> io::Result<Vec<u8>> {
    let mut out = Vec::new();
    if let Some(mut p) = pipe {
        p.read_to_end(&mut out).await?;
    }
    Ok(out)
}

/// A running command. It derefs to `tokio::process::Child`; dropping it before the command was reaped kills its group.
pub struct Child {
    inner: tokio::process::Child,
}

impl Deref for Child {
    type Target = tokio::process::Child;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Child {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        // id() is None once the leader was reaped: its pid, and so the group's id, may then be another process's, and
        // nothing is killed: only a command still running is cancelled.
        // A pid past i32 is not one the kernel hands out; there is then no group to kill.
        if let Some(pid) = self
            .inner
            .id()
            .and_then(|id| i32::try_from(id).ok())
            .and_then(Pid::from_raw)
        {
            #[expect(
                clippy::let_underscore_must_use,
                reason = "drop has no caller to tell; ESRCH means the group already ended, which is what was wanted"
            )]
            let _ = kill_process_group(pid, Signal::KILL);
        }
    }
}
