//! The stages of a session's start, each bounded on its own, so a start that hangs fails with the stage that hung and a
//! start the provider fails carries the provider's failure to failover, rather than one clock over the whole start that
//! hides both.

use std::future::Future;
use std::time::Duration;

use wire::runtime::{Failure, SESSION_OPEN_WAIT};

use crate::failover::provider_fault;

/// Why a session did not start: what the owner reads, and the provider's failure when the provider is at fault (failover
/// answers it).
pub type StartFailure = (String, Option<Failure>);

/// A stage of a session's start, after its account is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartStage {
    /// What the session is launched with is made: its worktree, prompt, tools and access (`Domains::spec`).
    Prepare,
    /// The AI runtime opens the session and Claude starts in it.
    Open,
}

impl StartStage {
    /// How long the stage may take. An open waits longer than the runtime gives Claude to start, so the runtime's
    /// answer (a `Service` failure when Claude did not start) arrives before the core gives up on it.
    pub const fn wait(self) -> Duration {
        match self {
            Self::Prepare => Duration::from_secs(3 * 60),
            Self::Open => SESSION_OPEN_WAIT.saturating_add(Duration::from_secs(30)),
        }
    }

    /// The stage, as the reason of a start that failed in it begins.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Prepare => "prepare the session",
            Self::Open => "open the session",
        }
    }
}

/// Runs stage of a start within wait (`StartStage::wait`, a test's shorter one): its own failure, named by the stage,
/// when it fails; the stage that gave no answer when time runs out. A stage that hangs past its wait is no provider's
/// fault the core can see, so it carries none.
pub async fn within<T>(
    stage: StartStage,
    wait: Duration,
    work: impl Future<Output = Result<T, StartFailure>>,
) -> Result<T, StartFailure> {
    match tokio::time::timeout(wait, work).await {
        Ok(Ok(done)) => Ok(done),
        Ok(Err((why, fault))) => Err((format!("{}: {why}", stage.label()), fault)),
        Err(_) => Err((format!("{}: no answer within {wait:?}", stage.label()), None)),
    }
}

/// Opens the session (the launcher's start) within wait: what failed it, named as the open, with the provider's failure
/// when the provider is at fault, so failover can answer it.
pub async fn opened<T>(wait: Duration, open: impl Future<Output = Result<T, agent::Error>>) -> Result<T, StartFailure> {
    within(StartStage::Open, wait, async {
        open.await.map_err(|e| (e.to_string(), provider_fault(&e)))
    })
    .await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use wire::runtime::FailureCategory;

    use super::*;
    use crate::health::ProbeHandler;

    /// A launcher as the core sees one through the AI runtime: it answers the open after `answers_after`, with `answer`,
    /// or never.
    struct Runtime {
        answers_after: Duration,
        answer: Option<Failure>,
    }

    impl agent::Launcher for Runtime {
        fn start(
            &self,
            _spec: agent::Spec,
            _h: Arc<dyn agent::Handler>,
        ) -> agent::BoxFuture<'_, Result<Box<dyn agent::Session>, agent::Error>> {
            Box::pin(async move {
                let Some(f) = self.answer.clone() else {
                    return std::future::pending().await;
                };
                tokio::time::sleep(self.answers_after).await;
                Err(agent::Error::Failure(f))
            })
        }
    }

    fn open(rt: &Runtime) -> impl Future<Output = Result<Box<dyn agent::Session>, agent::Error>> + '_ {
        agent::Launcher::start(rt, agent::Spec::default(), Arc::new(ProbeHandler::default()))
    }

    #[tokio::test(start_paused = true)]
    async fn start_timeout_names_the_stage_that_hung() {
        let hangs = Runtime {
            answers_after: Duration::ZERO,
            answer: None,
        };
        let (why, fault) = opened(StartStage::Open.wait(), open(&hangs)).await.err().unwrap();
        assert_eq!(why, "open the session: no answer within 210s");
        assert_eq!(fault, None);
        let (why, _) = within::<()>(StartStage::Prepare, StartStage::Prepare.wait(), std::future::pending())
            .await
            .unwrap_err();
        assert_eq!(why, "prepare the session: no answer within 180s");
    }

    /// The runtime gives Claude `SESSION_OPEN_WAIT` from when it takes the open, a moment after the core asked: its
    /// verdict reaches the core, and failover gets the provider's failure (launch answers it when there is one).
    #[tokio::test(start_paused = true)]
    async fn start_timeout_lets_the_runtimes_verdict_reach_failover() {
        let claude = Failure {
            category: FailureCategory::Service,
            title: "Claude did not start within 180s".to_owned(),
            resets_at: None,
        };
        let rt = Runtime {
            answers_after: SESSION_OPEN_WAIT + Duration::from_secs(1),
            answer: Some(claude.clone()),
        };
        let (why, fault) = opened(StartStage::Open.wait(), open(&rt)).await.err().unwrap();
        assert_eq!(why, "open the session: Claude did not start within 180s");
        assert_eq!(fault, Some(claude), "failover gets the provider's failure");
    }
}
