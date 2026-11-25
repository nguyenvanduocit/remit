use std::path::PathBuf;

use model::{Checks, ChecksState};
use serde::Deserialize;

use crate::forge::{Account, Cli, Error, Repo, nullable, raw};

/// Speaks to GitLab through glab, logged in on the machine.
#[derive(Debug, Clone)]
pub struct GitLab {
    cli: Cli,
}

/// A branch, as GitLab answers with it.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GlBranch {
    #[serde(deserialize_with = "nullable")]
    commit: GlCommit,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GlCommit {
    #[serde(deserialize_with = "nullable")]
    id: String,
    /// The pipeline GitLab ran last on the commit; none when it ran none.
    last_pipeline: Option<GlPipeline>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GlPipeline {
    #[serde(deserialize_with = "nullable")]
    id: i64,
    #[serde(deserialize_with = "nullable")]
    status: String,
}

/// A merge request's state, as GitLab answers with it: opened, merged, closed or locked.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GlMR {
    #[serde(deserialize_with = "nullable")]
    state: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GlUser {
    #[serde(deserialize_with = "nullable")]
    username: String,
    #[serde(deserialize_with = "nullable")]
    name: String,
    #[serde(deserialize_with = "nullable")]
    email: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GlJob {
    #[serde(deserialize_with = "nullable")]
    name: String,
}

fn project_of(r: &Repo) -> String {
    format!("projects/{}", urlescape::path_segment(&r.project))
}

impl GitLab {
    /// GitLab through the glab program bin.
    pub fn new(bin: impl Into<PathBuf>) -> Self {
        Self {
            cli: Cli {
                bin: bin.into(),
                name: "gitlab",
            },
        }
    }

    pub fn name(&self) -> &'static str {
        "GitLab"
    }

    pub async fn account(&self, host: &str) -> Result<Account, Error> {
        let answer = self.cli.get(host, "user", &[]).await?;
        let u = serde_json::from_slice::<GlUser>(&answer)
            .map_err(|_| self.cli.unexpected(format!("user on {host}"), &answer))?;
        Ok(Account {
            login: u.username,
            name: u.name,
            email: u.email,
        })
    }

    /// The commit branch stands at on GitLab.
    pub async fn branch_head(&self, r: &Repo, branch: &str) -> Result<String, Error> {
        let path = format!(
            "{}/repository/branches/{}",
            project_of(r),
            urlescape::path_segment(branch)
        );
        let answer = self.cli.get(&r.host, &path, &[]).await?;
        serde_json::from_slice::<GlBranch>(&answer)
            .ok()
            .map(|b| b.commit.id)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| self.cli.unexpected(format!("branch {branch}"), &answer))
    }

    /// How the pipeline GitLab ran last on sha stands; none when it ran none.
    pub async fn checks(&self, r: &Repo, sha: &str) -> Result<Option<Checks>, Error> {
        let path = format!("{}/repository/commits/{sha}", project_of(r));
        let answer = self.cli.get(&r.host, &path, &[]).await?;
        let c = serde_json::from_slice::<GlCommit>(&answer)
            .ok()
            .filter(|c| !c.id.is_empty())
            .ok_or_else(|| self.cli.unexpected(format!("commit {sha}"), &answer))?;
        match c.last_pipeline {
            Some(p) => self.pipeline_checks(r, p.id, &p.status).await,
            None => Ok(None),
        }
    }

    /// Maps a GitLab pipeline's status to how its commit's checks stand; a failed one names its first failed job.
    async fn pipeline_checks(&self, r: &Repo, pipeline: i64, status: &str) -> Result<Option<Checks>, Error> {
        let failed = |name: String| {
            Some(Checks {
                state: ChecksState::Failed,
                failed: name,
            })
        };
        Ok(match status {
            "skipped" | "" => None,
            "success" => Some(Checks {
                state: ChecksState::Passed,
                failed: String::new(),
            }),
            "canceled" => failed("pipeline canceled".to_owned()),
            "failed" => {
                let path = format!("{}/pipelines/{pipeline}/jobs?scope=failed", project_of(r));
                let answer = self.cli.get(&r.host, &path, &[]).await?;
                let jobs = serde_json::from_slice::<Option<Vec<GlJob>>>(&answer).map_err(|_| {
                    self.cli
                        .unexpected(format!("the failed jobs of pipeline {pipeline}"), &answer)
                })?;
                let first = jobs
                    .unwrap_or_default()
                    .into_iter()
                    .next()
                    .map(|j| j.name)
                    .filter(|name| !name.is_empty());
                failed(first.unwrap_or_else(|| "pipeline failed".to_owned()))
            }
            // created, pending, running, preparing, waiting_for_resource, scheduled, manual
            _ => Some(Checks {
                state: ChecksState::Pending,
                failed: String::new(),
            }),
        })
    }

    /// Closes merge request iid with comment, once: one no longer open is left as it is, and says false. Written for the
    /// one-time close of the draft merge requests Remit opened before it landed changes itself (migration 0119), and
    /// deleted with it.
    pub async fn close_mr(&self, r: &Repo, iid: i64, comment: &str) -> Result<bool, Error> {
        let path = format!("{}/merge_requests/{iid}", project_of(r));
        let answer = self.cli.get(&r.host, &path, &[]).await?;
        let mr = serde_json::from_slice::<GlMR>(&answer)
            .ok()
            .filter(|mr| !mr.state.is_empty())
            .ok_or_else(|| self.cli.unexpected(format!("MR !{iid}"), &answer))?;
        if mr.state != "opened" {
            return Ok(false);
        }
        let notes = format!("{path}/notes");
        self.cli
            .call(&r.host, "POST", &notes, &[raw("body", comment)], &[])
            .await?;
        self.cli
            .call(&r.host, "PUT", &path, &[raw("state_event", "close")], &[])
            .await?;
        Ok(true)
    }
}
