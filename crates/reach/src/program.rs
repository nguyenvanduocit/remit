//! The run tool's executor (ADR-0018, form T2): the source crate decides what may run and builds the job; this runs it in a
//! jail. The credential is in the job's environment and goes nowhere else: not into the jail's arguments, not into a file,
//! and what the program wrote is cleaned of it by the source crate before an agent reads it.
//!
//! A program gets no network. The one exception is a job that lists addresses (`Job::egress`): it gets the way out ADR-0018
//! allows, a gate of its own (crates/egress) that connects it to those addresses and to nothing else, made for that call and
//! closed when it ends. A job that names a connection to reach (`Job::inject`, a script: script.rs) gets the same way out with
//! the injecting proxy behind it instead.

use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use egress::{Config, Denial, Gate, Injector};
use source::{BoxError, BoxFuture, Job, Output, Spawned};
use telemetry::Child;
use tracing::Instrument as _;

/// Bytes of data a program may hold: a bundled interpreter needs room.
const PROGRAM_MEMORY: u64 = 2 << 30;
const PROGRAM_CPU: Duration = Duration::from_secs(20);

const GATE_SOCKET_NAME: &str = "gate.sock";
/// Bytes one tunnel may carry, both directions together.
const GATE_MAX_BYTES: u64 = 64 << 20;
/// Tunnels open at once.
pub(crate) const GATE_MAX_TUNNELS: usize = 16;

/// Runs a source's program in a jail: the `source::Runner` the server gives its sources.
#[derive(Clone)]
pub struct JailRunner {
    /// The authorities a script's connection is checked against (script.rs); none is the system's.
    pub(crate) upstream_roots: Option<Arc<rustls::RootCertStore>>,
    /// Dates the certificates of a script's proxy: the orchestrator's clock.
    pub(crate) now: Arc<dyn Fn() -> SystemTime + Send + Sync>,
}

impl JailRunner {
    /// A runner whose scripts check their connection against the system's authorities, on the system clock.
    #[must_use]
    pub fn new() -> JailRunner {
        JailRunner {
            upstream_roots: None,
            now: Arc::new(SystemTime::now),
        }
    }
}

impl Default for JailRunner {
    fn default() -> Self {
        JailRunner::new()
    }
}

impl JailRunner {
    /// Runs j in a jail with env, j's environment as its child span told it, through what j reaches: a connection's
    /// proxy, a gate, or nothing.
    async fn run_jail(&self, j: &Job, env: Vec<String>) -> Result<Output, BoxError> {
        let mut read_only = jail::system_paths();
        read_only.extend(j.read_only.iter().cloned());
        let spec = jail::Spec {
            argv: j.argv.clone(),
            dir: j.dir.to_string_lossy().into_owned(),
            env,
            read_only,
            timeout: j.timeout,
            max_output: j.max_output,
            memory_bytes: PROGRAM_MEMORY,
            cpu: PROGRAM_CPU,
            ..jail::Spec::default()
        };
        match (&j.inject, j.egress.is_empty()) {
            (Some(_), false) => {
                Err("a job reaches addresses through a gate or through a connection's proxy, not both".into())
            }
            (Some(_), true) => self.run_through_injector(j, spec).await,
            (None, false) => run_through_gate(j, spec).await,
            (None, true) => run_jailed(spec).await,
        }
    }
}

impl source::Runner for JailRunner {
    /// Runs j to its end in a span of its own under the work that runs it (`remit.child`), which the program is told of
    /// (ADR-0119) and which records how it ended.
    fn run<'a>(&'a self, j: &'a Job) -> BoxFuture<'a, Result<Output, BoxError>> {
        let child = Child::open(j.argv.first().map_or("", String::as_str), "");
        let ran = self.run_jail(j, child.tell(j.env.clone()));
        let span = child.span().clone();
        Box::pin(
            async move {
                match ran.await {
                    Ok(out) => {
                        child.ended(out.exit_code, out.timed_out);
                        Ok(out)
                    }
                    Err(err) => {
                        child.failed();
                        Err(err)
                    }
                }
            }
            .instrument(span),
        )
    }

    /// Starts an MCP server over stdio (ADR-0077) in a jail with the job's environment, and, when it lists addresses, a gate
    /// to them kept open as long as the program runs. Dropping what it returns ends both.
    fn spawn<'a>(&'a self, j: &'a Job) -> BoxFuture<'a, Result<Spawned, BoxError>> {
        Box::pin(async move {
            if j.inject.is_some() {
                return Err(
                    "a long-lived program reaches its addresses through a gate, not a connection's proxy".into(),
                );
            }
            let mut read_only = jail::system_paths();
            read_only.extend(j.read_only.iter().cloned());
            let mut spec = jail::Spec {
                argv: j.argv.clone(),
                dir: j.dir.to_string_lossy().into_owned(),
                env: j.env.clone(),
                read_only,
                timeout: j.timeout,
                max_output: j.max_output.max(1),
                memory_bytes: PROGRAM_MEMORY,
                cpu: j.timeout,
                ..jail::Spec::default()
            };
            let mut gate = None;
            if !j.egress.is_empty() {
                let dir = tempfile::Builder::new()
                    .prefix("remit-gate-")
                    .tempdir()
                    .map_err(|e| format!("egress gate: {e}"))?;
                let g = Gate::new(
                    &dir.path().join(GATE_SOCKET_NAME),
                    Config {
                        allow: j.egress.clone(),
                        methods: Vec::new(),
                        deadline: SERVER_TUNNEL,
                        max_bytes: GATE_MAX_BYTES,
                        max_tunnels: GATE_MAX_TUNNELS,
                        resolver: None,
                    },
                )
                .await
                .map_err(|e| format!("egress gate: {e}"))?;
                spec.socket = Some(Gate::socket(&g).to_string_lossy().into_owned());
                spec.read_only.extend(jail::ca_paths());
                gate = Some((g, dir));
            }
            let mut jailed = jail::spawn(spec).await?;
            let (Some(stdin), Some(stdout)) = (jailed.child.stdin.take(), jailed.child.stdout.take()) else {
                return Err("the jailed program has no stdio".into());
            };
            // The program goes first, then its gate: a drop in field order.
            let hold: Box<dyn Send> = Box::new((jailed, gate));
            Ok(Spawned {
                stdin: Box::new(stdin),
                stdout: Box::new(stdout),
                hold,
            })
        })
    }
}

/// How long one tunnel of a long-lived program may stay open: a server's request is a call's.
const SERVER_TUNNEL: Duration = Duration::from_secs(10 * 60);

/// What a jail's socket leads to: a gate, or the injecting proxy, which is a gate that injects.
pub(crate) trait ProxyEnd: Send + Sync {
    fn socket(&self) -> String;
    fn denied(&self) -> Vec<Denial>;
    fn close(&self) -> BoxFuture<'_, ()>;
}

impl ProxyEnd for Gate {
    fn socket(&self) -> String {
        Gate::socket(self).to_string_lossy().into_owned()
    }
    fn denied(&self) -> Vec<Denial> {
        Gate::denied(self)
    }
    fn close(&self) -> BoxFuture<'_, ()> {
        Box::pin(Gate::close(self))
    }
}

impl ProxyEnd for Injector {
    fn socket(&self) -> String {
        Gate::socket(self).to_string_lossy().into_owned()
    }
    fn denied(&self) -> Vec<Denial> {
        Gate::denied(self)
    }
    fn close(&self) -> BoxFuture<'_, ()> {
        Box::pin(Gate::close(self))
    }
}

/// Runs spec with a gate that lets the program reach j.egress, made for this call and closed when it ends.
async fn run_through_gate(j: &Job, spec: jail::Spec) -> Result<Output, BoxError> {
    run_behind(spec, |socket, mut spec| async move {
        let gate = Gate::new(
            &socket,
            Config {
                allow: j.egress.clone(),
                methods: Vec::new(),
                deadline: j.timeout,
                max_bytes: GATE_MAX_BYTES,
                max_tunnels: GATE_MAX_TUNNELS,
                resolver: None,
            },
        )
        .await?;
        // The certificate authorities a program checks TLS against: one without a gate verifies nothing.
        let mut read_only = jail::system_paths();
        read_only.extend(jail::ca_paths());
        read_only.extend(j.read_only.iter().cloned());
        spec.read_only = read_only;
        let end: Box<dyn ProxyEnd> = Box::new(gate);
        Ok((end, spec))
    })
    .await
}

/// Runs spec with the proxy that open makes (and may set the jail's environment and paths for), made for this call and
/// closed when it ends. The proxy's socket is in a directory of its own, not in the program's work directory: the jail shows
/// that one read-write and mounts the socket over its own path, which a path inside it cannot take.
pub(crate) async fn run_behind<F, Fut>(spec: jail::Spec, open: F) -> Result<Output, BoxError>
where
    F: FnOnce(std::path::PathBuf, jail::Spec) -> Fut,
    Fut: Future<Output = Result<(Box<dyn ProxyEnd>, jail::Spec), BoxError>>,
{
    let dir = tempfile::Builder::new()
        .prefix("remit-gate-")
        .tempdir()
        .map_err(|e| format!("egress gate: {e}"))?;
    let (proxy, mut spec) = open(dir.path().join(GATE_SOCKET_NAME), spec)
        .await
        .map_err(|e| format!("egress gate: {e}"))?;
    spec.socket = Some(proxy.socket());
    let ran = run_jailed(spec).await;
    // The jail has ended, so no tunnel is left to serve: after close the refusals are final.
    proxy.close().await;
    let mut out = ran?;
    out.stderr.push_str(&refusal_note(&proxy.denied()));
    Ok(out)
}

async fn run_jailed(spec: jail::Spec) -> Result<Output, BoxError> {
    let res = jail::run(spec).await?;
    Ok(Output {
        stdout: res.stdout,
        stderr: res.stderr,
        exit_code: res.exit_code,
        timed_out: res.timed_out,
    })
}

/// env minus the variables of those names.
pub(crate) fn without(env: &[String], names: &[&str]) -> Vec<String> {
    env.iter()
        .filter(|e| !names.contains(&e.split_once('=').map_or(e.as_str(), |(n, _)| n)))
        .cloned()
        .collect()
}

/// Says, after the program's own output, what the gate refused, so that an agent can tell a blocked address from a failing
/// program. Empty when the gate refused nothing.
fn refusal_note(denied: &[Denial]) -> String {
    if denied.is_empty() {
        return String::new();
    }
    let items: Vec<String> = denied
        .iter()
        .map(|d| {
            // most reasons name the target; the others need it said
            let mut item = if d.reason.contains(&d.target) {
                d.reason.clone()
            } else {
                format!("{}: {}", d.target, d.reason)
            };
            if d.count > 1 {
                item.push_str(&format!(" ({} times)", d.count));
            }
            item
        })
        .collect();
    format!("\n[Remit: the egress gate refused {}]", items.join("; "))
}

/// Whether a path is the jail's to show: one of the system's paths or under one.
pub(crate) fn under_system_paths(path: &Path) -> bool {
    jail::system_paths()
        .iter()
        .any(|root| path == Path::new(root) || path.starts_with(root))
}
