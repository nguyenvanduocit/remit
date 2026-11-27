//! How a release's steps run on this server (ADR-0116): shipping's `StepRunner` over `procgroup::Program` and reach's
//! injecting proxy. A step with no connection runs as it is; one with connections reaches each through the grant its
//! repository holds, behind one proxy that adds the credentials (reach's release_step.rs).

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use procgroup::{Limits, Program, Ran};
use reach::Reach;
use reach::program::ProgramRunner;
use shipping::release::Connection;
use shipping::release_run::StepRunner;

/// How a release's steps run, as the server's configuration says.
#[derive(Clone, Default)]
pub enum ReleaseSteps {
    /// None runs here: a release waits, saying so.
    #[default]
    None,
    /// As programs of this server, through the injecting proxy to the connections its repository holds grants on.
    Programs(ProgramRunner),
    /// A runner the caller made (a test's, which runs nothing outside).
    Given(Arc<dyn StepRunner>),
}

/// The steps of a release as programs of this server, reaching through reach.
pub(crate) struct ProgramSteps {
    pub reach: Arc<Reach>,
    pub runner: ProgramRunner,
}

#[async_trait]
impl StepRunner for ProgramSteps {
    async fn run_step(
        &self,
        repo: &str,
        connections: &[Connection],
        home: &Path,
        program: Program,
        limits: Limits,
    ) -> Result<Ran, String> {
        if connections.is_empty() {
            return program.run(limits).await.map_err(|e| e.to_string());
        }
        let named: Vec<(String, String)> = connections
            .iter()
            .map(|c| (c.source.clone(), c.connection.clone()))
            .collect();
        let way = self.reach.release_way(repo, &named).await?;
        let out = self
            .runner
            .run_release_step(program, home, &way, limits)
            .await
            .map_err(|e| way.scrubbed(&e.to_string()))?;
        Ok(Ran {
            stdout: out.stdout,
            stderr: out.stderr,
            exit_code: out.exit_code,
            timed_out: out.timed_out,
        })
    }
}
