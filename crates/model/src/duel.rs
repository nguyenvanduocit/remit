use serde::{Deserialize, Serialize};

use crate::Time;

/// Two lineages at the same work (docs/research-agent-learning/design.md, D10): the Chair contested a card, so a sibling
/// card with the same brief went to an employee hired from another template, and a judge Remit starts picks the better
/// report once both have reported. The pick is a duel decision the owner may overturn. Ratings of a lineage are computed
/// from the duels, never stored beside them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Duel {
    pub id: String,
    pub chain: String,
    /// The kind of work it was fought in: its chain's kind (`ChainKind`).
    pub work: String,
    /// The contested card first, then its sibling.
    pub sides: [DuelSide; 2],
    /// The judge's card, hired once both sides reported.
    pub judge: String,
    /// The card whose report won as it stands: the judge's pick, or the other side once the owner overturned it.
    pub winner: String,
    /// The owner's overturn of the judge's pick: the decision that records it.
    pub overturn: String,
    /// Why the duel ended without a pick (a side ended without a report, the judge gave none); its reports then wait for
    /// the Chair.
    pub ended: String,
    pub created_at: Time,
    /// Counts its stored changes: each write is made over the revision it read, so none is lost.
    pub rev: i64,
}

/// One card of a duel and the employee and lineage at work in it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DuelSide {
    pub card: String,
    pub employee: String,
    pub template: String,
}

impl Duel {
    /// The side whose card is card_id, when there is one.
    pub fn side(&self, card_id: &str) -> Option<&DuelSide> {
        self.sides.iter().find(|s| s.card == card_id)
    }

    /// The side facing the card card_id.
    pub fn other(&self, card_id: &str) -> &DuelSide {
        let [first, second] = &self.sides;
        if first.card == card_id { second } else { first }
    }

    /// Whether the judge has picked a winner.
    pub fn judged(&self) -> bool {
        !self.winner.is_empty()
    }
}
