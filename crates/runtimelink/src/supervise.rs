//! The AI runtime's process, started again when it exits: only the first run decides whether the core can run agents.

use std::sync::Arc;
use std::time::Duration;

use agent::BoxFuture;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::Error;
use crate::convert::Ending;
use crate::launcher::{Launcher, Runtime};
use crate::port::Link;

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// One run of the AI runtime, as `supervise` started it.
pub trait Process: Send + Sync + 'static {
    /// Returns once the runtime has exited, with why when it failed.
    fn wait(&self) -> BoxFuture<'_, Result<(), BoxError>>;
    /// Ends the runtime and every process it started.
    fn kill(&self) -> Result<(), BoxError>;
}

/// The process `supervise` runs, and the runtime it said hello as once it has.
pub(crate) struct Supervised<L> {
    n: u64,
    process: Arc<dyn Process>,
    rt: Option<Arc<Runtime<L>>>,
}

/// After the first exit the runtime starts again after `FIRST_BACKOFF`, doubling to `MAX_BACKOFF`, and back to
/// `FIRST_BACKOFF` for a runtime that stayed up `STEADY_AFTER`. `STOP_WAIT` is how long a runtime told to end may take
/// to close its sessions before it is killed.
const FIRST_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);
const STEADY_AFTER: time::Duration = time::Duration::minutes(1);
const STOP_WAIT: Duration = Duration::from_secs(3);

/// How one run ended.
struct Run<L> {
    /// The runtime it said hello as; none when it did not.
    rt: Option<Arc<Runtime<L>>>,
    /// When it said hello; none when it did not.
    up_at: Option<model::Time>,
    /// `life` ended it.
    stopped: bool,
    why: String,
}

impl<L: Link> Launcher<L> {
    /// Runs the AI runtime `start` makes, and starts it again when it exits, until `life` is cancelled. A first runtime
    /// that cannot be started, or exits before its hello, fails `supervise` with why, as a start that cannot run agents
    /// fails. Once a runtime has said hello, every later one that cannot be started or exits is started again after a
    /// backoff. When `life` is cancelled, the process has been told to end (`start` gives it the token); `supervise`
    /// returns once it has exited.
    pub async fn supervise<F>(&self, life: &CancellationToken, start: F) -> Result<(), Error>
    where
        F: Fn(CancellationToken) -> BoxFuture<'static, Result<Arc<dyn Process>, BoxError>>,
    {
        let mut backoff = FIRST_BACKOFF;
        let mut greeted = false;
        loop {
            let run = self.supervise_one(life, &start).await;
            if run.stopped {
                return Ok(());
            }
            if !greeted && run.rt.is_none() {
                return Err(Error::Runtime(run.why));
            }
            greeted = true;
            let steady = run
                .up_at
                .and_then(|up| up.0.checked_add(STEADY_AFTER))
                .is_some_and(|at| at <= self.now().0);
            if run.rt.is_some() && steady {
                backoff = FIRST_BACKOFF;
            }
            tracing::warn!(error = %run.why, after = ?backoff, "the AI runtime ended: it starts again");
            tokio::select! {
                () = tokio::time::sleep(backoff) => {}
                () = life.cancelled() => return Ok(()),
            }
            backoff = backoff.saturating_mul(2).min(MAX_BACKOFF);
            let mut shared = self.shared();
            shared.state.restarts = shared.state.restarts.saturating_add(1);
        }
    }

    /// Starts one runtime and returns once it has exited.
    async fn supervise_one<F>(&self, life: &CancellationToken, start: &F) -> Run<L>
    where
        F: Fn(CancellationToken) -> BoxFuture<'static, Result<Arc<dyn Process>, BoxError>>,
    {
        let hellos = self.shared().hellos;
        let process = match start(life.clone()).await {
            Ok(process) => process,
            Err(e) => {
                return Run {
                    rt: None,
                    up_at: None,
                    stopped: life.is_cancelled(),
                    why: format!("start the AI runtime: {e}"),
                };
            }
        };
        let n = {
            let mut shared = self.shared();
            shared.next_proc = shared.next_proc.wrapping_add(1);
            shared.proc = Some(Supervised {
                n: shared.next_proc,
                process: Arc::clone(&process),
                rt: None,
            });
            shared.next_proc
        };
        let waiting = Arc::clone(&process);
        let exited = tokio::spawn(async move { waiting.wait().await });
        let (rt, up_at, stopped, exit) = self.watch(life, process.as_ref(), n, hellos, exited).await;
        self.runtime_gone(n, rt.as_deref(), &exit);
        let why = match &rt {
            None => format!("the AI runtime exited before it connected: {exit}"),
            Some(rt) => format!("the AI runtime {} exited: {exit}", rt.id),
        };
        Run {
            rt,
            up_at,
            stopped,
            why,
        }
    }

    /// Waits for the process to exit, noting the runtime it says hello as (the first hello after `hellos`) and when.
    /// When `life` is cancelled it gives the process `STOP_WAIT` to exit, then kills it.
    async fn watch(
        &self,
        life: &CancellationToken,
        process: &dyn Process,
        n: u64,
        hellos: u64,
        mut exited: JoinHandle<Result<(), BoxError>>,
    ) -> (Option<Arc<Runtime<L>>>, Option<model::Time>, bool, String) {
        let mut rt = None;
        let mut up_at = None;
        loop {
            let mut changed = self.subscribe();
            if rt.is_none() {
                let now = self.now();
                let mut shared = self.shared();
                if shared.hellos > hellos
                    && let Some(cur) = shared.rt.clone()
                {
                    if let Some(proc) = shared.proc.as_mut().filter(|p| p.n == n) {
                        proc.rt = Some(Arc::clone(&cur));
                    }
                    rt = Some(cur);
                    up_at = Some(now);
                }
            }
            tokio::select! {
                exit = &mut exited => return (rt, up_at, life.is_cancelled(), exit_text(exit)),
                _ = changed.changed() => {}
                () = life.cancelled() => {
                    let exit = match tokio::time::timeout(STOP_WAIT, &mut exited).await {
                        Ok(exit) => exit,
                        Err(_) => {
                            if let Err(e) = process.kill() {
                                tracing::warn!(error = %e, "kill the AI runtime that did not end");
                            }
                            (&mut exited).await
                        }
                    };
                    return (rt, up_at, true, exit_text(exit));
                }
            }
        }
    }

    /// Ends what the runtime `rt` of process `n` held: its process exited, so every session in it ends as the host's
    /// restart, and every sign-in in it ends.
    fn runtime_gone(&self, n: u64, rt: Option<&Runtime<L>>, exit: &str) {
        let now = self.now();
        {
            let mut shared = self.shared();
            if shared.proc.as_ref().is_some_and(|p| p.n == n) {
                shared.proc = None;
            }
            if let Some(rt) = rt {
                rt.held().conn = None;
                if shared.rt.as_ref().is_some_and(|cur| std::ptr::eq(cur.as_ref(), rt)) {
                    shared.rt = None;
                }
            }
            if shared.rt.is_none() {
                shared.state.connected = false;
                shared.state.reason = format!("the AI runtime exited: {exit}");
                shared.state.since = now;
            }
        }
        self.bump();
        if let Some(rt) = rt {
            self.end(
                rt,
                &Ending::Restarted(format!("the AI runtime {} exited ({exit})", rt.id)),
            );
        }
    }

    /// Kills the process of runtime `rt`, when `supervise` runs it: a session it would not end ends with it. False when
    /// no supervised process holds `rt`.
    pub(crate) fn kill_runtime(&self, rt: &Runtime<L>) -> bool {
        let process = self
            .shared()
            .proc
            .as_ref()
            .filter(|p| p.rt.as_ref().is_some_and(|held| std::ptr::eq(held.as_ref(), rt)))
            .map(|p| Arc::clone(&p.process));
        let Some(process) = process else { return false };
        if let Err(e) = process.kill() {
            tracing::warn!(runtime = %rt.id, error = %e, "kill the AI runtime");
        }
        true
    }
}

/// How a runtime exited, said in words.
fn exit_text(exit: Result<Result<(), BoxError>, tokio::task::JoinError>) -> String {
    match exit {
        Ok(Ok(())) => "it exited with nothing to say".to_owned(),
        Ok(Err(e)) => e.to_string(),
        Err(e) => format!("its wait ended abnormally: {e}"),
    }
}
