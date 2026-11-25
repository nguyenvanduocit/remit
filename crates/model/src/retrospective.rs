use serde::{Deserialize, Serialize};

use crate::text_enum;
use crate::{Actor, Time};

text_enum! {
    /// How much an issue cost the chain.
    #[derive(Default)]
    pub enum Impact {
        High = "high",
        #[default]
        Medium = "medium",
        Low = "low",
    }
}

/// What the coach wrote after a chain finished (ADR-0082): what went wrong, which memories its sessions recalled helped
/// or misled, and the brief that would have served better. One per finish, so a chain that finishes again has another;
/// chains finish without bound, so it is a record of its own beside the chain, as a decision is.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Retrospective {
    pub id: String,
    pub chain_id: String,
    /// The coach's Chair that wrote it.
    pub by: Actor,
    pub issues: Vec<RetroIssue>,
    pub memories: Vec<RetroMemory>,
    /// The brief the chain should have started from; "Start again with this brief" starts a chain from it.
    pub brief: String,
    /// The change item the coach's lesson path raised from it; none when it raised none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lesson: Option<String>,
    pub created_at: Time,
    /// Counts its stored changes: of two copies, a channel keeps the one with the higher.
    pub rev: i64,
}

/// Something that went wrong in a chain, in words, with what it cost.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RetroIssue {
    pub impact: Impact,
    pub text: String,
}

/// A memory a session of the chain recalled, and whether it helped or misled the work.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RetroMemory {
    pub memory_id: String,
    /// The memory's words as the coach read them, kept so the retrospective reads the same after the memory changes.
    pub text: String,
    pub helped: bool,
    pub why: String,
}
