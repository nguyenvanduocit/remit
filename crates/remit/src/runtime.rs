//! The AI runtime (ADR-0058): `remit runtime`, the one process that runs Claude Code sessions, started by the core as
//! its child with its settings on stdin, and dialling the core back on the link with the key minted for this start. The
//! key and the certificate's hash never travel in an argument or the environment, where another process could read
//! them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex, PoisonError};

use agent::BoxFuture;
use runtimelink::{BoxError, Peer, Process};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::config::AgentFlags;
use crate::rotate::{AGENTS_LOG_SIZE, RotatingFile};

/// The subcommand the core starts its AI runtime with.
pub const COMMAND: &str = "runtime";

/// What the core hands the runtime it starts: one JSON line on the runtime's stdin, which then stays open as the core's
/// lifeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// The core's `/link` URL.
    pub core_url: String,
    /// The SHA-256 hash of the certificate the core's link presents this start, hex.
    pub certificate_hash: String,
    /// Minted for this start of the core.
    pub key: String,
    /// The claude command and the agents' config dir.
    pub agents: AgentFlags,
    /// -claude-config-dir: the login linked into the agents' config dir on Linux.
    pub claude_login: String,
    /// `<data>/agents.log`, where the sessions' stderr goes.
    pub agents_log: String,
}

/// `remit runtime`: reads its settings from stdin, makes the agents' config dir ready and runs every session the core
/// opens, until stdin closes (the core is gone) or the core refuses its key. It ignores the signals a terminal or a
/// supervisor sends the whole group: it ends after the core, which closes its sessions first.
pub async fn run(offset: time::UtcOffset) -> Result<(), String> {
    // Held for the life of the process: a signal with a handler does not end it.
    let _interrupt =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()).map_err(|e| e.to_string())?;
    let _terminate =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).map_err(|e| e.to_string())?;
    let mut stdin = BufReader::new(tokio::io::stdin());
    let mut line = String::new();
    stdin
        .read_line(&mut line)
        .await
        .map_err(|e| format!("read the settings the core hands over: {e}"))?;
    let s: Settings = serde_json::from_str(&line).map_err(|e| format!("read the settings the core hands over: {e}"))?;
    let life = CancellationToken::new();
    let closed = life.clone();
    tokio::spawn(async move {
        // Any end of the pipe, clean or not, is the core's end.
        let mut sink = Vec::new();
        stdin.read_to_end(&mut sink).await.ok();
        closed.cancel();
    });
    let log = RotatingFile::open(PathBuf::from(&s.agents_log), AGENTS_LOG_SIZE)
        .map_err(|e| format!("{}: {e}", s.agents_log))?;
    let home = PathBuf::from(&s.agents.claude_home);
    crate::agent_home::prepare(&home, Path::new(&s.claude_login))?;
    tracing::info!(dir = %home.display(), login = %s.claude_login, "agents' claude config");
    let hash = hex::decode(&s.certificate_hash).map_err(|e| format!("the core's certificate hash: {e}"))?;
    let client = link_quic::client(link_quic::Trust::CertificateHashes(vec![hash])).map_err(|e| e.to_string())?;
    let url = link_quic::Url::parse(&s.core_url).map_err(|e| format!("the core's URL {}: {e}", s.core_url))?;
    let accounts = s.agents.accounts();
    let stderr: claudecode::Stderr = Arc::new(Mutex::new(log));
    let cfg = airuntime::Config {
        key: s.key.clone(),
        revision: crate::health::this_build().revision,
        launcher: Arc::new(launcher(&s.agents, stderr)),
        accounts: Some(Arc::new(accounts)),
        transcripts: Some(airuntime::TranscriptDir(home.join("projects"))),
        now: crate::clock::model_clock(offset),
    };
    airuntime::run(life, QuicDial { client, url }, cfg)
        .await
        .map_err(|e| e.to_string())
}

/// Starts every session on Claude Code, in the config dir of its account.
fn launcher(f: &AgentFlags, stderr: claudecode::Stderr) -> agent::ClaudeLauncher {
    let accounts = f.accounts();
    agent::ClaudeLauncher {
        command: f.command(),
        home: Some(Arc::new(move |id: &str| accounts.home(id).display().to_string())),
        env: Vec::new(),
        stderr: Some(stderr),
        now: Arc::new(jiff::Timestamp::now),
        store: Arc::new(claudecode::Store::of),
    }
}

/// Dials the core on the link, trusting the certificate whose hash it was handed.
struct QuicDial {
    client: link_quic::Client,
    url: link_quic::Url,
}

impl airuntime::Dial for QuicDial {
    type Link = link_quic::Link;

    async fn dial(&self, hello: link::Hello) -> Result<Peer<link_quic::Link>, BoxError> {
        let peer = link_quic::dial(&self.client, self.url.clone(), link::Config::default(), hello).await?;
        Ok(peer.into())
    }
}

/// How the core starts its runtime: this binary again as `exe runtime`, its settings on stdin, its stderr beside the core's.
/// The runtime's stdin closes when life ends, which ends the runtime.
pub fn starter(
    exe: PathBuf,
    s: Settings,
) -> impl Fn(CancellationToken) -> BoxFuture<'static, Result<Arc<dyn Process>, BoxError>> + Send + Sync + 'static {
    move |life| {
        let (exe, s) = (exe.clone(), s.clone());
        Box::pin(async move {
            let p: Arc<dyn Process> = Arc::new(start(&exe, &s, life).await?);
            Ok(p)
        })
    }
}

async fn start(exe: &Path, s: &Settings, life: CancellationToken) -> Result<Child, BoxError> {
    let mut line = serde_json::to_vec(s)?;
    line.push(b'\n');
    #[expect(
        clippy::disallowed_methods,
        reason = "the AI runtime is Remit's own binary, the half that runs the sessions: it takes the server's environment and its life is held here (Child::kill), not by a process group"
    )]
    let mut child = tokio::process::Command::new(exe)
        .arg(COMMAND)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("start {} {COMMAND}: {e}", exe.display()))?;
    let pid = child
        .id()
        .and_then(|id| i32::try_from(id).ok())
        .ok_or("the AI runtime has no pid")?;
    let mut stdin = child.stdin.take().ok_or("the AI runtime has no stdin")?;
    let stderr = child.stderr.take().ok_or("the AI runtime has no stderr")?;
    let tail = Arc::new(Mutex::new(Tail::default()));
    let copying = tokio::spawn(copy_stderr(stderr, Arc::clone(&tail)));
    let (exited_tx, exited) = watch::channel(None);
    let ended = CancellationToken::new();
    let ended_by_exit = ended.clone();
    tokio::spawn(async move {
        let status = child.wait().await.map_err(|e| e.to_string());
        copying.await.ok(); // its last words are in the tail before the exit is told
        ended_by_exit.cancel();
        exited_tx.send_replace(Some(status));
    });
    if let Err(e) = stdin.write_all(&line).await {
        let p = Child { pid, exited, tail };
        p.kill().ok();
        return Err(format!("hand the AI runtime its settings: {e}").into());
    }
    // The core's end, or the runtime's exit: either way its stdin closes once.
    tokio::spawn(async move {
        tokio::select! {
            () = life.cancelled() => {}
            () = ended.cancelled() => {}
        }
        drop(stdin);
    });
    Ok(Child { pid, exited, tail })
}

/// Copies the runtime's stderr to the core's, keeping its end.
async fn copy_stderr(mut from: tokio::process::ChildStderr, tail: Arc<Mutex<Tail>>) {
    let mut out = tokio::io::stderr();
    let mut buf = vec![0_u8; 8 << 10];
    loop {
        let n = match from.read(&mut buf).await {
            Ok(0) | Err(_) => return,
            Ok(n) => n,
        };
        let Some(chunk) = buf.get(..n) else { return };
        out.write_all(chunk).await.ok();
        tail.lock().unwrap_or_else(PoisonError::into_inner).push(chunk);
    }
}

/// The end of what a process wrote: its last words when it exits.
#[derive(Default)]
struct Tail(Vec<u8>);

const TAIL_KEPT: usize = 2 << 10;

impl Tail {
    fn push(&mut self, b: &[u8]) {
        self.0.extend_from_slice(b);
        let over = self.0.len().saturating_sub(TAIL_KEPT);
        self.0.drain(..over);
    }
}

/// The AI runtime the core started.
struct Child {
    pid: i32,
    /// Its exit, once it has exited.
    exited: watch::Receiver<Option<Result<std::process::ExitStatus, String>>>,
    tail: Arc<Mutex<Tail>>,
}

impl Process for Child {
    /// Returns once the runtime has exited; a failed exit says the last of what it wrote.
    fn wait(&self) -> BoxFuture<'_, Result<(), BoxError>> {
        let mut exited = self.exited.clone();
        Box::pin(async move {
            let status = exited
                .wait_for(Option::is_some)
                .await
                .map_err(|_| "the AI runtime's wait ended")?
                .clone()
                .ok_or("the AI runtime's wait ended")?;
            match status {
                Ok(s) if s.success() => Ok(()),
                Ok(s) => {
                    let tail = self.tail.lock().unwrap_or_else(PoisonError::into_inner);
                    Err(format!("{s}: {}", String::from_utf8_lossy(&tail.0).trim()).into())
                }
                Err(e) => Err(e.into()),
            }
        })
    }

    /// Ends the runtime and every process it started. Each Claude Code session leads its own process group, so killing
    /// the runtime alone would leave them running: the runtime is stopped first, so it starts nothing more, then each
    /// process under it is killed with its group, deepest first, then the runtime.
    fn kill(&self) -> Result<(), BoxError> {
        use rustix::process::{Pid, Signal, kill_process, kill_process_group};
        let pid = Pid::from_raw(self.pid).ok_or("the AI runtime has no pid")?;
        let gone = |r: rustix::io::Result<()>| match r {
            Err(rustix::io::Errno::SRCH) => Ok(()),
            other => other,
        };
        gone(kill_process(pid, Signal::STOP)).map_err(|e| format!("stop the AI runtime: {e}"))?;
        #[expect(
            clippy::disallowed_methods,
            reason = "ps lists the runtime's descendants to stop them; it is the machine's own tool and reads nothing it could pass on"
        )]
        let listed = std::process::Command::new("ps")
            .args(["-A", "-o", "pid=,ppid="])
            .output();
        let walked = match listed {
            Ok(out) if out.status.success() => parse_ps(&String::from_utf8_lossy(&out.stdout)).map(|parents| {
                for d in descendants(&parents, self.pid) {
                    if let Some(d) = Pid::from_raw(d) {
                        // Most are not group leaders, and one gone is ended.
                        kill_process_group(d, Signal::KILL).ok();
                        kill_process(d, Signal::KILL).ok();
                    }
                }
            }),
            Ok(out) => Err(format!("list the processes under the AI runtime: ps {}", out.status)),
            Err(e) => Err(format!("list the processes under the AI runtime: {e}")),
        };
        gone(kill_process(pid, Signal::KILL)).map_err(|e| format!("kill the AI runtime: {e}"))?;
        walked.map_err(Into::into)
    }
}

/// The pids under root in the tree parents gives (pid to its parent), deepest first, root not among them. Pure.
fn descendants(parents: &HashMap<i32, i32>, root: i32) -> Vec<i32> {
    let mut children: HashMap<i32, Vec<i32>> = HashMap::new();
    for (&pid, &parent) in parents {
        if pid != parent {
            children.entry(parent).or_default().push(pid);
        }
    }
    let mut out = Vec::new();
    let mut stack = vec![(root, false)];
    while let Some((pid, seen)) = stack.pop() {
        if seen {
            if pid != root {
                out.push(pid);
            }
            continue;
        }
        stack.push((pid, true));
        for &c in children.get(&pid).into_iter().flatten() {
            stack.push((c, false));
        }
    }
    out
}

/// Reads `ps -A -o pid=,ppid=`: one process a line, its pid and its parent's. Pure.
fn parse_ps(out: &str) -> Result<HashMap<i32, i32>, String> {
    let mut parents = HashMap::new();
    for (n, line) in out.lines().enumerate() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        match fields.as_slice() {
            [] => {}
            [pid, parent] => {
                let num = |s: &str| {
                    s.parse::<i32>()
                        .map_err(|e| format!("ps line {}: {e}", n.saturating_add(1)))
                };
                parents.insert(num(pid)?, num(parent)?);
            }
            _ => {
                return Err(format!(
                    "ps line {} is not a pid and a parent: {line:?}",
                    n.saturating_add(1)
                ));
            }
        }
    }
    Ok(parents)
}
