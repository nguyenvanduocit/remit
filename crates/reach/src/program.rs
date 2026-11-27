//! The run tool's executor (ADR-0018, form T2): the source crate decides what may run and builds the job; this runs it
//! through `procgroup::Program`, the one runner (ADR-0138). The credential is in the job's environment and goes nowhere
//! else: not into the program's arguments, not into a file, and what the program wrote is cleaned of it by the source crate
//! before an agent reads it.
//!
//! A job that lists addresses (`Job::egress`) gets a gate of its own (crates/egress) that connects it to those addresses
//! and to nothing else, made for that call and closed when it ends, and its proxy variables point at it. A job that names a
//! connection to reach (`Job::inject`, a script: script.rs) gets the injecting proxy instead. A job with neither gets no
//! proxy.

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use egress::{Config, Denial, Gate, Injector};
use procgroup::{Limits, Program};
use source::{BoxError, BoxFuture, Job, Output, Spawned};
use telemetry::Child;
use tracing::Instrument as _;

/// Bytes one tunnel may carry, both directions together.
const GATE_MAX_BYTES: u64 = 64 << 20;
/// Tunnels open at once.
pub(crate) const GATE_MAX_TUNNELS: usize = 16;

/// Runs a source's programs, scripts and stdio servers, and a release's steps: the `source::Runner` the server gives its
/// sources.
#[derive(Clone)]
pub struct ProgramRunner {
    /// The authorities a script's connection is checked against (script.rs); none is the system's.
    pub(crate) upstream_roots: Option<Arc<rustls::RootCertStore>>,
    /// Dates the certificates of a script's proxy: the orchestrator's clock.
    pub(crate) now: Arc<dyn Fn() -> SystemTime + Send + Sync>,
}

impl ProgramRunner {
    /// A runner whose scripts check their connection against the system's authorities, on the system clock.
    #[must_use]
    pub fn new() -> ProgramRunner {
        ProgramRunner {
            upstream_roots: None,
            now: Arc::new(SystemTime::now),
        }
    }
}

impl Default for ProgramRunner {
    fn default() -> Self {
        ProgramRunner::new()
    }
}

/// The program of j with env, and no proxy yet.
fn program_of(j: &Job, env: Vec<String>) -> Program {
    Program {
        argv: j.argv.clone(),
        dir: j.dir.clone(),
        env,
        proxy: None,
    }
}

impl ProgramRunner {
    /// Runs j with env, j's environment as its child span told it, through what j reaches: a connection's proxy, a gate,
    /// or nothing.
    async fn run_job(&self, j: &Job, env: Vec<String>) -> Result<Output, BoxError> {
        let program = program_of(j, env);
        match (&j.inject, j.egress.is_empty()) {
            (Some(_), false) => {
                Err("a job reaches addresses through a gate or through a connection's proxy, not both".into())
            }
            (Some(_), true) => self.run_through_injector(j, program).await,
            (None, false) => run_through_gate(j, program).await,
            (None, true) => run_to_end(&program, limits_of(j)).await,
        }
    }
}

impl source::Runner for ProgramRunner {
    /// Runs j to its end in a span of its own under the work that runs it (`remit.child`), which the program is told of
    /// (ADR-0119) and which records how it ended.
    fn run<'a>(&'a self, j: &'a Job) -> BoxFuture<'a, Result<Output, BoxError>> {
        let child = Child::open(j.argv.first().map_or("", String::as_str), "");
        let ran = self.run_job(j, child.tell(j.env.clone()));
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

    /// Starts an MCP server over stdio (ADR-0077) with the job's environment, and, when it lists addresses, a gate to them
    /// kept open as long as the program runs. Dropping what it returns ends both.
    fn spawn<'a>(&'a self, j: &'a Job) -> BoxFuture<'a, Result<Spawned, BoxError>> {
        Box::pin(async move {
            if j.inject.is_some() {
                return Err(
                    "a long-lived program reaches its addresses through a gate, not a connection's proxy".into(),
                );
            }
            let mut program = program_of(j, j.env.clone());
            let mut gate = None;
            if !j.egress.is_empty() {
                let g = Gate::new(Config {
                    allow: j.egress.clone(),
                    methods: Vec::new(),
                    deadline: SERVER_TUNNEL,
                    max_bytes: GATE_MAX_BYTES,
                    max_tunnels: GATE_MAX_TUNNELS,
                    resolver: None,
                })
                .await
                .map_err(|e| format!("egress gate: {e}"))?;
                program.proxy = Some(g.addr());
                gate = Some(g);
            }
            let mut child = program.start()?;
            let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
                return Err("the program has no stdio".into());
            };
            // The program goes first, then its gate: a drop in field order.
            let hold: Box<dyn Send> = Box::new((child, gate));
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

/// What a program's proxy variables lead to: a gate, or the injecting proxy, which is a gate that injects.
pub(crate) trait ProxyEnd: Send + Sync {
    fn addr(&self) -> SocketAddr;
    fn denied(&self) -> Vec<Denial>;
    fn close(&self) -> BoxFuture<'_, ()>;
}

impl ProxyEnd for Gate {
    fn addr(&self) -> SocketAddr {
        Gate::addr(self)
    }
    fn denied(&self) -> Vec<Denial> {
        Gate::denied(self)
    }
    fn close(&self) -> BoxFuture<'_, ()> {
        Box::pin(Gate::close(self))
    }
}

impl ProxyEnd for Injector {
    fn addr(&self) -> SocketAddr {
        Gate::addr(self)
    }
    fn denied(&self) -> Vec<Denial> {
        Gate::denied(self)
    }
    fn close(&self) -> BoxFuture<'_, ()> {
        Box::pin(Gate::close(self))
    }
}

/// Runs program with a gate that lets it reach j.egress, made for this call and closed when it ends.
async fn run_through_gate(j: &Job, program: Program) -> Result<Output, BoxError> {
    run_behind(program, limits_of(j), |program| async move {
        let gate = Gate::new(Config {
            allow: j.egress.clone(),
            methods: Vec::new(),
            deadline: j.timeout,
            max_bytes: GATE_MAX_BYTES,
            max_tunnels: GATE_MAX_TUNNELS,
            resolver: None,
        })
        .await?;
        let end: Box<dyn ProxyEnd> = Box::new(gate);
        Ok((end, program))
    })
    .await
}

/// Runs program under limits with the proxy that open makes (and may set the program's environment for), made for this
/// call and closed when it ends.
pub(crate) async fn run_behind<F, Fut>(program: Program, limits: Limits, open: F) -> Result<Output, BoxError>
where
    F: FnOnce(Program) -> Fut,
    Fut: Future<Output = Result<(Box<dyn ProxyEnd>, Program), BoxError>>,
{
    let (proxy, mut program) = open(program).await.map_err(|e| format!("egress gate: {e}"))?;
    program.proxy = Some(proxy.addr());
    let ran = run_to_end(&program, limits).await;
    // The program has ended, so no tunnel is left to serve: after close the refusals are final.
    proxy.close().await;
    let mut out = ran?;
    out.stderr.push_str(&refusal_note(&proxy.denied()));
    Ok(out)
}

/// The limits j runs under.
pub(crate) fn limits_of(j: &Job) -> Limits {
    Limits {
        timeout: j.timeout,
        max_output: j.max_output,
    }
}

async fn run_to_end(program: &Program, limits: Limits) -> Result<Output, BoxError> {
    let ran = program.run(limits).await?;
    Ok(Output {
        stdout: ran.stdout,
        stderr: ran.stderr,
        exit_code: ran.exit_code,
        timed_out: ran.timed_out,
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
