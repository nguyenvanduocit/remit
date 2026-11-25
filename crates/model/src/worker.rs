use serde::{Deserialize, Serialize};

use crate::Time;

/// A code the owner had made for a mentor: the key a browser of theirs signs in to the web app with. A mentor holds as many
/// as they work from devices, each made on the owner's words and revoked on its own (ADR-0060). The key is no part of
/// this record: the server keeps it sealed apart from it, and only the owner is shown it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorkerKey {
    pub id: String,
    /// The mentor it opens the channel for.
    pub mentor: String,
    /// What it is for, in the owner's words: "Lan's phone".
    pub note: String,
    /// When and from where it last opened the channel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<Time>,
    pub last_addr: String,
    pub created_at: Time,
    /// Counts its stored changes.
    pub rev: i64,
}
