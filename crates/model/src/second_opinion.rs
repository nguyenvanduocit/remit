use serde::{Deserialize, Serialize};

use crate::Time;

/// A second judge's re-decision of one of the Assistant's answers for the owner (ADR-0103): Remit starts a fresh judge on
/// a random share of the answers, which decides the item without seeing the Assistant's answer, then reads it and says
/// whether the two decide the same. How often they differ is a figure the coach reads (overturn_rates); a difference is
/// a dissent decision the owner reads beside the answer he may overturn.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SecondOpinion {
    pub id: String,
    /// The chain whose item it is: the judge works there.
    pub chain: String,
    /// The Assistant's answer it re-decides: an assist decision.
    pub decision: String,
    /// The inbox item both decided.
    pub item: String,
    /// The judge's card.
    pub judge: String,
    /// The judge's own answer, given before it read the Assistant's, and why.
    pub answer: String,
    /// The option it chose, by id, when its item lists options: what the deciders' picks are scored against (ADR-0118,
    /// point 5).
    pub option: String,
    /// Whether it was started because the Assistant's answer parted from a decider, and not by the random share. The
    /// picks it scores are not a sample of the deciders' answers, so no standing is computed from them
    /// (`OutcomeSource::SecondOpinionTargeted`).
    pub targeted: bool,
    /// Whether it judged that answering the item could be undone later at little cost; none when it was not asked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub undoable: Option<bool>,
    pub why: String,
    /// Whether the two decide the same, once the judge compared them; none before.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agrees: Option<bool>,
    /// The judge's words on how the two compare.
    pub compared: String,
    /// Why it ended without a comparison (the judge's turn ended or failed first).
    pub ended: String,
    pub created_at: Time,
    /// Counts its stored changes: each write is made over the revision it read, so none is lost.
    pub rev: i64,
}

impl SecondOpinion {
    /// Whether the judge has given its own answer.
    pub fn answered(&self) -> bool {
        !self.answer.is_empty()
    }

    /// Whether nothing more comes of it: compared, or ended without.
    pub fn over(&self) -> bool {
        self.agrees.is_some() || !self.ended.is_empty()
    }
}
