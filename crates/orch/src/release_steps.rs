//! How a release's steps run on this server (ADR-0116): shipping's `StepRunner` over the jail and reach's injecting
//! proxy. A step with no connection runs in the jail with no network; one with connections reaches each through the
//! grant its repository holds, behind one proxy that adds the credentials (reach's release_step.rs).

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use reach::Reach;
use reach::program::JailRunner;
use shipping::release::Connection;
use shipping::release_run::StepRunner;

/// How a release's steps run, as the server's configuration says.
#[derive(Clone, Default)]
pub enum ReleaseSteps {
    /// None runs here: a release waits, saying so.
    #[default]
    None,
    /// In the jail, through the injecting proxy to the connections its repository holds grants on.
    Jailed(JailRunner),
    /// A runner the caller made (a test's, which runs nothing outside).
    Given(Arc<dyn StepRunner>),
}

/// The steps of a release in the jail, reaching through reach.
pub(crate) struct JailedSteps {
    pub reach: Arc<Reach>,
    pub runner: JailRunner,
}

#[async_trait]
impl StepRunner for JailedSteps {
    async fn run_step(
        &self,
        repo: &str,
        connections: &[Connection],
        home: &Path,
        spec: jail::Spec,
    ) -> Result<jail::RunResult, String> {
        if connections.is_empty() {
            return jail::run(spec).await.map_err(|e| e.to_string());
        }
        let named: Vec<(String, String)> = connections
            .iter()
            .map(|c| (c.source.clone(), c.connection.clone()))
            .collect();
        let way = self.reach.release_way(repo, &named).await?;
        let deadline = spec.timeout;
        let out = self
            .runner
            .run_release_step(spec, home, &way, deadline)
            .await
            .map_err(|e| way.scrubbed(&e.to_string()))?;
        Ok(jail::RunResult {
            stdout: out.stdout,
            stderr: out.stderr,
            exit_code: out.exit_code,
            timed_out: out.timed_out,
        })
    }
}
