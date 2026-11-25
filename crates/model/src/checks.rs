use serde::{Deserialize, Serialize};

use crate::text_enum;

text_enum! {
    /// How the checks a forge runs on a commit stand: still running, all passed, or one failed.
    pub enum ChecksState {
        Pending = "pending",
        Passed = "passed",
        Failed = "failed",
    }
}

/// How the checks a forge runs on one commit stand, and the one that failed when one did. A commit the forge runs none on
/// has no Checks at all (none), so "none" and "passed" are never the same value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Checks {
    pub state: ChecksState,
    /// Names the failed check (a job, a check run, a status context) when `state` is failed.
    #[serde(default)]
    pub failed: String,
}
