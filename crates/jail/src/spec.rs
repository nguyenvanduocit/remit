//! The spec of one program to run in a jail, what it left behind, and the one check of a spec.

use std::path::Path;
use std::time::Duration;

/// Why a jail could not be built, a program could not be started in one, or a helper could not run.
#[derive(Debug, thiserror::Error)]
#[error("jail: {0}")]
pub struct Error(pub(crate) String);

impl Error {
    pub(crate) fn new(what: impl std::fmt::Display) -> Self {
        Self(what.to_string())
    }
}

/// One program to run in a jail. Every field is the whole of what the program gets of that kind.
#[derive(Debug, Clone, Default)]
pub struct Spec {
    /// The program and its arguments. A bare `argv[0]` is looked up in the jail, along the PATH of `env`.
    pub argv: Vec<String>,
    /// The work directory, where the program starts, and which it can write but the `read_only` paths inside it.
    pub dir: String,
    /// The other paths the program can write, but the `read_only` paths inside them, each a directory the host has: a
    /// HOME or TMPDIR of its own, outside the work directory.
    pub writable: Vec<String>,
    /// The whole environment of the program, but for the proxy variables a `socket` sets: nothing is inherited.
    pub env: Vec<String>,
    /// The host paths the program sees, read-only, at the same path (a path that does not exist is skipped). One inside
    /// `dir` or a `writable` path stays read-only, and the directories above it there cannot be renamed.
    pub read_only: Vec<String>,
    /// Paths, inside `read_only` or `dir`, that cannot be read.
    pub hide: Vec<String>,
    /// A unix socket of the host the program can connect to: its only way out. The jail relays a port of 127.0.0.1 to
    /// it and sets the proxy variables to that port, in place of any `env` holds. Without one nothing is reachable.
    pub socket: Option<String>,
    /// Ends the program, and everything it started, when it passes.
    pub timeout: Duration,
    /// How many bytes of each of stdout and stderr are kept.
    pub max_output: usize,
    /// Limits the program's data segment and private mappings (RLIMIT_DATA). Linux applies it; macOS refuses every
    /// memory rlimit, so there the timeout and CPU bound the program instead (ADR-0021).
    pub memory_bytes: u64,
    /// Limits the processor time the program may use (RLIMIT_CPU, in whole seconds, rounded up).
    pub cpu: Duration,
}

/// What a program left behind.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunResult {
    /// Cut at `Spec::max_output` bytes each, with a marker saying so.
    pub stdout: String,
    pub stderr: String,
    /// The program's, 128 plus the signal when a signal ended it, and -1 when the jail ended it.
    pub exit_code: i32,
    /// The jail ended the program because `Spec::timeout` passed.
    pub timed_out: bool,
}

impl Spec {
    /// Says whether the jail can honour the spec. It looks at nothing but the spec.
    pub fn validate(&self) -> Result<(), Error> {
        let Some(argv0) = self.argv.first() else {
            return Err(Error::new("argv is empty"));
        };
        if argv0.is_empty() || (argv0.contains('/') && !Path::new(argv0).is_absolute()) {
            return Err(Error::new(format!(
                "argv[0] {argv0:?} is neither a bare name to look up nor an absolute path"
            )));
        }
        if self.argv.iter().chain(&self.env).any(|a| a.contains('\0')) {
            return Err(Error::new("an argument or environment entry holds a NUL byte"));
        }
        absolute("dir", &self.dir)?;
        if self.dir == "/" {
            return Err(Error::new("dir is the root, the one path a program may write"));
        }
        for e in &self.env {
            if e.find('=').is_none_or(|i| i == 0) {
                return Err(Error::new(format!("env entry {e:?} has no name before =")));
            }
        }
        absolute_all("writable", &self.writable)?;
        if self.writable.iter().any(|p| p == "/") {
            return Err(Error::new("writable path / would open the whole host"));
        }
        absolute_all("read-only", &self.read_only)?;
        if self.read_only.iter().any(|p| p == "/") {
            return Err(Error::new("read-only path / would show the whole host"));
        }
        absolute_all("hide", &self.hide)?;
        if let Some(s) = &self.socket {
            absolute("socket", s)?;
        }
        self.limits()
    }

    /// Checks that every program has a deadline, an output cap and a memory and processor limit.
    fn limits(&self) -> Result<(), Error> {
        if self.timeout.is_zero() {
            return Err(Error::new("timeout must be positive: every program has a deadline"));
        }
        if self.max_output == 0 {
            return Err(Error::new("max output must be positive"));
        }
        if self.memory_bytes == 0 {
            return Err(Error::new("memory limit must be set"));
        }
        if self.cpu.is_zero() {
            return Err(Error::new("cpu limit must be positive"));
        }
        Ok(())
    }

    /// The processor limit in the whole seconds RLIMIT_CPU counts, rounded up.
    pub(crate) fn cpu_seconds(&self) -> u64 {
        self.cpu
            .as_secs()
            .saturating_add(u64::from(self.cpu.subsec_nanos() > 0))
    }
}

/// Checks one path of the named kind: absolute and clean, so that what is validated is what is mounted.
pub(crate) fn absolute(kind: &str, p: &str) -> Result<(), Error> {
    if !p.starts_with('/') || clean(p) != p || p.contains('\0') {
        return Err(Error::new(format!("{kind} path {p:?} is not absolute and clean")));
    }
    Ok(())
}

pub(crate) fn absolute_all(kind: &str, paths: &[String]) -> Result<(), Error> {
    paths.iter().try_for_each(|p| absolute(kind, p))
}

/// The lexical clean of an absolute path: no empty, `.` or `..` element, no trailing slash.
fn clean(p: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for part in p.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            _ => out.push(part),
        }
    }
    format!("/{}", out.join("/"))
}
