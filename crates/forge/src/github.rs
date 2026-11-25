use std::path::PathBuf;

use model::{Checks, ChecksState};
use serde::Deserialize;

use crate::forge::{Account, Cli, Error, Repo, nullable, raw};

/// Speaks to GitHub (github.com or a GitHub Enterprise host) through gh, logged in on the machine.
#[derive(Debug, Clone)]
pub struct GitHub {
    cli: Cli,
}

/// A branch's ref, as GitHub answers with it.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GhRef {
    #[serde(deserialize_with = "nullable")]
    object: GhObject,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GhObject {
    #[serde(deserialize_with = "nullable")]
    sha: String,
}

/// A pull request's state, as GitHub answers with it: open or closed.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GhPull {
    #[serde(deserialize_with = "nullable")]
    state: String,
}

/// One check run of a commit, as GitHub answers with it.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GhRun {
    #[serde(deserialize_with = "nullable")]
    name: String,
    /// queued, in_progress or completed.
    #[serde(deserialize_with = "nullable")]
    status: String,
    /// Set once completed.
    #[serde(deserialize_with = "nullable")]
    conclusion: String,
}

/// One commit status, as GitHub answers with it.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GhStatus {
    #[serde(deserialize_with = "nullable")]
    context: String,
    /// pending, success, failure or error.
    #[serde(deserialize_with = "nullable")]
    state: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GhRuns {
    #[serde(deserialize_with = "nullable")]
    check_runs: Vec<GhRun>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GhCombined {
    #[serde(deserialize_with = "nullable")]
    statuses: Vec<GhStatus>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GhUser {
    #[serde(deserialize_with = "nullable")]
    id: i64,
    #[serde(deserialize_with = "nullable")]
    login: String,
    name: Option<String>,
    email: Option<String>,
}

/// Maps GitHub's check runs and commit statuses of one commit to how its checks stand: none of either is `None`; a run
/// that failed (or timed out, was cancelled, needs an action, never started) or a status that failed or errored fails them
/// and names the first such, in the order GitHub gave; otherwise a run not completed or a status pending leaves them
/// pending; otherwise they passed.
fn github_checks(runs: &[GhRun], statuses: &[GhStatus]) -> Option<Checks> {
    if runs.is_empty() && statuses.is_empty() {
        return None;
    }
    let failed = |name: &str| {
        Some(Checks {
            state: ChecksState::Failed,
            failed: name.to_owned(),
        })
    };
    let mut pending = false;
    for r in runs {
        if r.status != "completed" {
            pending = true;
        } else if matches!(
            r.conclusion.as_str(),
            "failure" | "timed_out" | "cancelled" | "action_required" | "startup_failure"
        ) {
            return failed(&r.name);
        }
    }
    for st in statuses {
        match st.state.as_str() {
            "failure" | "error" => return failed(&st.context),
            "pending" => pending = true,
            _ => {}
        }
    }
    Some(Checks {
        state: if pending {
            ChecksState::Pending
        } else {
            ChecksState::Passed
        },
        failed: String::new(),
    })
}

impl GitHub {
    /// GitHub through the gh program bin.
    pub fn new(bin: impl Into<PathBuf>) -> Self {
        Self {
            cli: Cli {
                bin: bin.into(),
                name: "github",
            },
        }
    }

    pub fn name(&self) -> &'static str {
        "GitHub"
    }

    /// The account gh is logged in as on host. GitHub gives an email only when the account made one public; the address
    /// GitHub keeps for every account's commits stands in for it.
    pub async fn account(&self, host: &str) -> Result<Account, Error> {
        let answer = self.cli.get(host, "user", &[]).await?;
        let u = serde_json::from_slice::<GhUser>(&answer)
            .ok()
            .filter(|u| !u.login.is_empty())
            .ok_or_else(|| self.cli.unexpected(format!("user on {host}"), &answer))?;
        let given = |v: Option<String>| v.filter(|v| !v.is_empty());
        Ok(Account {
            name: given(u.name).unwrap_or_else(|| u.login.clone()),
            email: given(u.email).unwrap_or_else(|| format!("{}+{}@users.noreply.{host}", u.id, u.login)),
            login: u.login,
        })
    }

    /// Reads the check runs and the commit statuses of sha and says how they stand together. The check runs come a page at a
    /// time, each page one JSON object of its own, so a failed run past the first hundred is read too.
    pub async fn checks(&self, r: &Repo, sha: &str) -> Result<Option<Checks>, Error> {
        let path = format!("repos/{}/commits/{sha}/check-runs?per_page=100", r.project);
        let answer = self.cli.get(&r.host, &path, &["--paginate"]).await?;
        let mut runs = Vec::new();
        for page in serde_json::Deserializer::from_slice(&answer).into_iter::<GhRuns>() {
            let page = page.map_err(|_| self.cli.unexpected(format!("the check runs of {sha}"), &answer))?;
            runs.extend(page.check_runs);
        }
        let path = format!("repos/{}/commits/{sha}/status", r.project);
        let answer = self.cli.get(&r.host, &path, &[]).await?;
        let combined = serde_json::from_slice::<GhCombined>(&answer)
            .map_err(|_| self.cli.unexpected(format!("the statuses of {sha}"), &answer))?;
        Ok(github_checks(&runs, &combined.statuses))
    }

    /// The commit branch stands at on GitHub.
    pub async fn branch_head(&self, r: &Repo, branch: &str) -> Result<String, Error> {
        let path = format!(
            "repos/{}/git/ref/heads/{}",
            r.project,
            urlescape::path_segment(branch).replace("%2F", "/")
        );
        let answer = self.cli.get(&r.host, &path, &[]).await?;
        serde_json::from_slice::<GhRef>(&answer)
            .ok()
            .map(|g| g.object.sha)
            .filter(|sha| !sha.is_empty())
            .ok_or_else(|| self.cli.unexpected(format!("branch {branch}"), &answer))
    }

    /// Closes pull request number with comment, once: one already closed is left as it is, and says false. Written for the
    /// one-time close of the draft pull requests Remit opened before it landed changes itself (migration 0119), and
    /// deleted with it.
    pub async fn close_mr(&self, r: &Repo, number: i64, comment: &str) -> Result<bool, Error> {
        let path = format!("repos/{}/pulls/{number}", r.project);
        let answer = self.cli.get(&r.host, &path, &[]).await?;
        let p = serde_json::from_slice::<GhPull>(&answer)
            .ok()
            .filter(|p| !p.state.is_empty())
            .ok_or_else(|| self.cli.unexpected(format!("pull request #{number}"), &answer))?;
        if p.state != "open" {
            return Ok(false);
        }
        let comments = format!("repos/{}/issues/{number}/comments", r.project);
        self.cli
            .call(&r.host, "POST", &comments, &[raw("body", comment)], &[])
            .await?;
        self.cli
            .call(&r.host, "PATCH", &path, &[raw("state", "closed")], &[])
            .await?;
        Ok(true)
    }
}
