//! The one runner every child program but a session goes through (ADR-0138): a source's program and script, a stdio MCP
//! server, a repository's checks and a release's steps. It runs the program with the environment it is given and
//! nothing of the server's, in its own process group, and points the program's HTTP clients at the proxy it names. Where
//! isolation would return, it returns here, as a wrapper around `Program::command`.

use std::net::SocketAddr;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::{ExitStatus, Stdio};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt};

use crate::procgroup::{Child, Command, Error, WAIT_DELAY, command, kill_group};

/// The variables that send a program's HTTP clients to its proxy. The runner owns them, as it alone knows the proxy's
/// address. NO_PROXY is set empty, so that no address is exempt from the proxy.
const PROXY_VARS: [&str; 6] = [
    "HTTPS_PROXY",
    "HTTP_PROXY",
    "https_proxy",
    "http_proxy",
    "NO_PROXY",
    "no_proxy",
];

/// One program to run. Every field is the whole of what the program gets of that kind.
#[derive(Debug, Clone, Default)]
pub struct Program {
    /// The program and its arguments. A bare `argv[0]` is looked up along the PATH of `env`.
    pub argv: Vec<String>,
    /// Where the program starts.
    pub dir: PathBuf,
    /// The whole environment of the program, `NAME=value` entries, but for the proxy variables `proxy` sets: nothing of
    /// the server's is inherited.
    pub env: Vec<String>,
    /// The HTTP proxy the program's clients are pointed at, in place of any proxy variable `env` holds: the egress gate or
    /// the injecting proxy. None sets no proxy variable.
    pub proxy: Option<SocketAddr>,
}

/// How long a program run to its end may take, and how much of its output is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Once it passes, the program and everything it started are killed.
    pub timeout: Duration,
    /// How many bytes of each of stdout and stderr are kept.
    pub max_output: usize,
}

/// What a program left behind.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ran {
    /// Cut at `Limits::max_output` bytes each, with a marker saying so.
    pub stdout: String,
    pub stderr: String,
    /// The program's, 128 plus the signal when a signal ended it, and -1 when the runner ended it.
    pub exit_code: i32,
    /// The runner ended the program because `Limits::timeout` passed.
    pub timed_out: bool,
}

impl Program {
    /// Runs the program to its end under limits and returns what it wrote and how it ended; once the timeout passes its
    /// whole group is killed. The error is for a program that could not be started; one that fails, or that the timeout
    /// ended, is a `Ran`. Dropping the future ends the program and everything it started.
    pub async fn run(&self, limits: Limits) -> Result<Ran, Error> {
        let Limits { timeout, max_output } = limits;
        let mut cmd = self.command()?;
        cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn()?;
        let (out, err) = (child.stdout.take(), child.stderr.take());
        // The pipes are read while the program runs: one left unread fills and blocks the program writing to it.
        let reads = async { tokio::join!(capped(out, max_output), capped(err, max_output)) };
        let deadline = tokio::time::sleep(timeout);
        tokio::pin!(reads, deadline);
        let mut read = None;
        let mut timed_out = false;
        let status = loop {
            tokio::select! {
                status = child.wait() => break status?,
                outputs = &mut reads, if read.is_none() => read = Some(outputs),
                () = &mut deadline, if !timed_out => {
                    timed_out = true;
                    kill_group(&child);
                }
            }
        };
        let (stdout, stderr) = match read {
            Some(outputs) => outputs,
            // A process the program left behind may still hold its pipes: they are read for WAIT_DELAY more at most.
            None => tokio::time::timeout(WAIT_DELAY, &mut reads).await.unwrap_or_default(),
        };
        Ok(Ran {
            stdout,
            stderr,
            exit_code: if timed_out { -1 } else { exit_code(status) },
            timed_out,
        })
    }

    /// Starts the program and leaves it running, its stdin and stdout piped to the caller and its stderr dropped: a
    /// server that answers on its stdio for as long as the caller holds the `Child` (an MCP server, ADR-0077).
    pub fn start(&self) -> Result<Child, Error> {
        let mut cmd = self.command()?;
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        cmd.spawn()
    }

    /// The command that starts the program: its own group's leader, in dir, with env and the proxy alone.
    fn command(&self) -> Result<Command, Error> {
        let Some((program, args)) = self.argv.split_first() else {
            return Err(Error::NoProgram);
        };
        let mut cmd = command(program, args);
        cmd.env_clear().current_dir(&self.dir);
        for (k, v) in self.environment().iter().filter_map(|e| e.split_once('=')) {
            cmd.env(k, v);
        }
        Ok(cmd)
    }

    /// env with every proxy variable set for proxy, and none of env's own: the first value of a variable is the one a C
    /// library reads, so a copy env held would win over the runner's.
    fn environment(&self) -> Vec<String> {
        let Some(proxy) = self.proxy else {
            return self.env.clone();
        };
        let url = format!("http://{proxy}");
        let mut out: Vec<String> = self
            .env
            .iter()
            .filter(|e| !PROXY_VARS.contains(&e.split_once('=').map_or(e.as_str(), |(k, _)| k)))
            .cloned()
            .collect();
        for name in PROXY_VARS {
            let value = if name.eq_ignore_ascii_case("NO_PROXY") {
                ""
            } else {
                &url
            };
            out.push(format!("{name}={value}"));
        }
        out
    }
}

/// How the process ended, as a shell says it: its status, or 128 plus the signal that ended it.
fn exit_code(s: ExitStatus) -> i32 {
    s.code()
        .or_else(|| s.signal().and_then(|sig| sig.checked_add(128)))
        .unwrap_or(-1)
}

/// Reads the pipe to its end, keeping the first `max` bytes and dropping the rest, so that the program writing to it is
/// never blocked by its own output.
async fn capped(pipe: Option<impl AsyncRead + Unpin>, max: usize) -> String {
    let mut kept = Vec::new();
    let mut cut = false;
    if let Some(mut p) = pipe {
        let mut buf = [0u8; 8192];
        while let Ok(n @ 1..) = p.read(&mut buf).await {
            let room = max.saturating_sub(kept.len());
            cut |= n > room;
            kept.extend_from_slice(buf.get(..n.min(room)).unwrap_or_default());
        }
    }
    let mut s = String::from_utf8_lossy(&kept).into_owned();
    if cut {
        s.push_str(&format!("\n[output cut at {max} bytes]\n"));
    }
    s
}
