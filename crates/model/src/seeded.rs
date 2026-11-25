use serde::{Deserialize, Serialize};

use crate::{Actor, Time};

/// A fault planted on purpose in a trial chain's work, so whether its judges catch it is a number (ADR-0102). The planter
/// says what is wrong and where, and only Remit keeps it: no judge's brief or tool reads it. One per chain at most, so a
/// verdict that sends the work back is a verdict on that fault. A record of its own beside the chain, as a
/// retrospective is, and it goes with the chain.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SeededFault {
    pub id: String,
    pub chain_id: String,
    /// The Chair that planted it.
    pub by: Actor,
    /// What is wrong, and why a judge should find it.
    pub fault: String,
    /// Where it is: a path and line of the chain's worktree.
    pub place: String,
    pub planted_at: Time,
}
