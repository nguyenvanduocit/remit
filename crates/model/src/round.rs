use serde::{Deserialize, Serialize};

use crate::{Serves, Time};

/// One round of competing attempts at a key result whose check is a command (ADR-0115): several chains serve it at once,
/// each an attempt of the round (`Chain::attempt`) in a worktree of its own, and no attempt's approved version enters its
/// queue by itself (`LandingState::Contending`). Once every attempt has an approved version or has finished, a judge
/// Remit starts runs the check on each approved head, reads them on the customer's side and picks one with
/// `pick_attempt`: the pick enters the queue, the others go back (`ReturnReason::NotPicked`), and the judge's reasons stay
/// on the round for the next. A key result runs rounds without bound, so a round is a record of its own.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Round {
    /// Derived from the key result and the number (`Round::id_of`): two openings of the same round store one.
    pub id: String,
    pub serves: Serves,
    /// Counts the key result's rounds: 1 for its first.
    pub number: i64,
    pub state: RoundState,
    pub created_at: Time,
    pub updated_at: Time,
    /// Counts its stored changes: each write is made over the revision it read, so none is lost.
    pub rev: i64,
}

impl Round {
    /// The id of round number of the key result s.
    pub fn id_of(s: &Serves, number: i64) -> String {
        format!("ro_{}_{}_{number}", s.objective, s.key_result)
    }

    /// Whether an attempt may still join it: no judge has been started on it.
    pub const fn open(&self) -> bool {
        matches!(self.state, RoundState::Open)
    }

    /// Its judge's card, once one was started.
    pub fn judge(&self) -> Option<&str> {
        match &self.state {
            RoundState::Open | RoundState::Ended { .. } => None,
            RoundState::Judging { judge } | RoundState::Picked { judge, .. } => Some(judge),
        }
    }
}

/// Where a round stands.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(remote = "Self", rename_all = "snake_case")]
pub enum RoundState {
    /// Its attempts are at work; another may join it.
    #[default]
    Open,
    /// Every attempt has an approved version or has finished, and the judge `judge` (its card) weighs them.
    Judging { judge: String },
    /// The judge picked the attempt `attempt` (its chain) for why; the owner may have overturned it since.
    Picked {
        judge: String,
        attempt: String,
        why: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        overturned: Option<PickOverturn>,
    },
    /// It ended without a pick, for why: no attempt had an approved version, or the judge gave none.
    Ended { why: String },
}

crate::json_tagged!(RoundState, "kind");

impl RoundState {
    /// Its kind, as the store's `state` column reads it.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Judging { .. } => "judging",
            Self::Picked { .. } => "picked",
            Self::Ended { .. } => "ended",
        }
    }
}

/// The owner's overturn of a round's pick: the decision that records it and his word, which the next round's attempts
/// read with the judge's reasons.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PickOverturn {
    pub decision: String,
    pub word: String,
}
