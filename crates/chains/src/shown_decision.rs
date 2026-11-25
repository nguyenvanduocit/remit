//! Which decisions are an outcome of a chain's work (ADR-0081), and a decision as a chain's stream reads it: the record
//! with that rule's answer beside it (`wire::app::DecisionRead`), so no channel keeps a list of its own. The answer is
//! computed when a decision is shown, never stored.

use model::{Decision, DecisionKind};
use wire::app::DecisionRead;

/// Whether a decision of kind k is an outcome of the chain's work, which its stream shows where it happened with who and
/// why: paused and resumed, an agent stopped, finished and reopened, a landing landed, sent back or taken back,
/// the change live, archived and unarchived. Pure.
pub fn is_outcome(k: &DecisionKind) -> bool {
    matches!(
        k,
        DecisionKind::Pause
            | DecisionKind::Resume
            | DecisionKind::Stop
            | DecisionKind::Finish
            | DecisionKind::Reopen
            | DecisionKind::Landing
            | DecisionKind::Live
            | DecisionKind::Archive
            | DecisionKind::Unarchive
    )
}

/// Decision d as a chain's stream reads it.
pub fn shown_decision(d: Decision) -> DecisionRead {
    let outcome = is_outcome(&d.kind);
    DecisionRead { decision: d, outcome }
}
