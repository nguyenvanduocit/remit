//! Running an arbitrary command in a view of the host with some paths hidden and some read-only.

use std::ffi::OsStr;
use std::ops::{Deref, DerefMut};

use crate::spec::absolute_all;
use crate::status::Status;
use crate::{Error, os_plan};

/// Runs a command in a view of the host in which some paths cannot be read and some cannot be written. The command keeps
/// the machine's network and its user id, so a Claude Code session can reach its model provider and the server's MCP
/// port while the key file, the database and the source env files are not there to read, and the server's own binary
/// and config are there to read and not to change.
#[derive(Debug, Clone)]
pub struct Wrap {
    hide: Vec<String>,
    read_only: Vec<String>,
}

/// What a Wrap changes in the command's view of the host.
#[derive(Debug, Clone, Default)]
pub struct Mask {
    /// Paths that cannot be read, written, renamed, deleted or, for a socket, connected to: on Linux a file reads as an
    /// empty file and a directory as an empty one, on macOS reading one is refused.
    pub hide: Vec<String>,
    /// Paths that stay readable where they are and cannot be written through that path. A path may not be both.
    pub read_only: Vec<String>,
}

/// The Wrap that applies a Mask. On Linux a path that does not exist when the command starts hides and protects nothing
/// (macOS holds the rule once the path is made), so list the directory that holds files made later; a hard link to a
/// hidden file, made elsewhere, is not hidden. No directory above a hidden or read-only path can be renamed or removed,
/// so no copy can be planted in its place (on Linux each such directory is a mount point of itself). On Linux a filesystem
/// mounted below a read-only directory is not made read-only with it.
pub fn hidden_with(m: Mask) -> Result<Wrap, Error> {
    if m.hide.is_empty() && m.read_only.is_empty() {
        return Err(Error::new("nothing to hide or protect"));
    }
    absolute_all("hide", &m.hide)?;
    absolute_all("read-only", &m.read_only)?;
    if let Some(p) = m.read_only.iter().find(|p| m.hide.contains(p)) {
        return Err(Error::new(format!("{p} is both hidden and read-only")));
    }
    Ok(Wrap {
        hide: m.hide,
        read_only: m.read_only,
    })
}

impl Wrap {
    /// The command that runs `program` with `args` inside the Wrap, as its own process group. Set it up as any
    /// `procgroup::Command` (environment, directory, stdio: they are the program's), then call [`Command::spawn`]. A bare
    /// program name is looked up along this process's PATH, as `procgroup::command` would.
    ///
    /// The program ends with everything it started, the processes that left its process group or session included
    /// (ADR-0069), when it exits on its own and when its `procgroup::Child` is dropped: dropping it is the stop.
    pub fn command<I, S>(&self, program: &str, args: I) -> Result<Command, Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let argv: Vec<String> = std::iter::once(program.to_owned())
            .chain(args.into_iter().map(|a| a.as_ref().to_string_lossy().into_owned()))
            .collect();
        let caller_path = std::env::var("PATH").map(|p| format!("PATH={p}")).unwrap_or_default();
        let path = crate::lookpath::look_path(program, &[caller_path], |_| true).map_err(Error)?;
        launch(&os_plan::wrap_plan(path, argv, &self.hide, &self.read_only))
    }
}

/// A command whose program is the helper, which sets the plan up and runs the program. It derefs to
/// `procgroup::Command` for the program's environment, directory and stdio.
pub struct Command {
    inner: procgroup::Command,
    status: Status,
}

impl Deref for Command {
    type Target = procgroup::Command;
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
    /// Starts the helper and returns once the program runs, or with why the helper could not run it.
    pub fn spawn(&mut self) -> Result<procgroup::Child, Error> {
        let mut child = self.inner.spawn().map_err(|e| Error::new(format!("start: {e}")))?;
        self.status.wait(|| match child.try_wait() {
            Ok(Some(s)) => Some(s.to_string()),
            Ok(None) => None,
            Err(e) => Some(e.to_string()),
        })?;
        Ok(child)
    }
}

/// The helper's command for a plan: this binary again, with the plan and the status socket in its arguments.
pub(crate) fn launch(plan: &os_plan::Plan) -> Result<Command, Error> {
    let status = Status::new()?;
    let body = serde_json::to_string(plan).map_err(|e| Error::new(format!("encode plan: {e}")))?;
    let inner = os_plan::helper_command(plan, &body, status.path())?;
    Ok(Command { inner, status })
}
