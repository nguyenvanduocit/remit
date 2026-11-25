//! What the chain core reads of a submit: whether one is under way, and which repository a job is the submit of. The
//! submit itself is shipping's (submit.rs there).

use model::{Chain, Event};
use serde::{Deserialize, Serialize};

/// Whether a submit of one of the chain's repositories is in progress.
pub fn submitting(c: &Chain) -> bool {
    c.repos.iter().any(|rw| rw.submit.is_some())
}

/// What a job records beside what it shows: the repository whose submit it is, which finds it again.
#[derive(Serialize, Deserialize)]
pub struct JobData {
    #[serde(default)]
    pub repo: String,
}

/// The repository whose submit job e is; empty for a job that is no submit's.
pub fn job_repo(e: &Event) -> String {
    e.data
        .as_ref()
        .and_then(|d| serde_json::from_value::<JobData>(d.0.clone()).ok())
        .map(|d| d.repo)
        .unwrap_or_default()
}
