use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Actor, Duration, Time};

/// The owner's settings, one record, changed by telling the Assistant (ADR-0059). Every value the owner set is a `Setting`:
/// it carries who set it and why, and one he left alone reads as its default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// How long a permission prompt waits for the lead or the owner before its time limit passes (ADR-0059: the Chair
    /// then decides alone, and a prompt with no Chair above it is refused); absent reads as 30.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_wait_minutes: Option<Setting<i64>>,
    /// The least share of a duel judge's picks the owner leaves standing, in a kind of work, for duels of that kind to go
    /// on; absent reads as 70, and 0 never pauses them. The coach's brief sets it
    /// (docs/research-agent-learning/design.md, D10).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duel_agreement_percent: Option<Setting<i64>>,
    /// How the owner steers each question Remit asks its deciders (ADR-0118, point 8), by the question's id without its
    /// version (`memory.same_claim`): a question's wording may change in a new version, what the owner trusts to answer
    /// it and how sure it must be do not.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub steering: BTreeMap<String, Steering>,
    pub rev: i64,
}

/// A value the owner set, with who set it and why: a setting that carries its own reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Setting<T> {
    pub value: T,
    pub by: Actor,
    pub why: String,
    pub at: Time,
}

/// What the owner set for one question Remit asks its deciders; a part he left alone is the question's own.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Steering {
    /// The decider that answers it first, a pinned model id, whatever the standings say.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decider: Option<Setting<String>>,
    /// The share of the answers it decides alone that must prove right, in percent (`TargetPrecision`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_precision_percent: Option<Setting<i64>>,
}

impl Steering {
    /// Whether the owner has left nothing set.
    pub const fn is_empty(&self) -> bool {
        self.decider.is_none() && self.target_precision_percent.is_none()
    }
}

/// A target precision as the owner may set it: a whole percent from `MIN` to `MAX`. Parsed once where it enters
/// (`parse`), so what is stored and read back as one is in range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TargetPrecision(u8);

impl TargetPrecision {
    /// Below it a decider as often wrong as right would decide alone.
    pub const MIN: u8 = 50;
    /// Above it is a precision no measurement can reach: 100 means never wrong.
    pub const MAX: u8 = 99;

    /// `percent` as a precision; none outside `MIN` to `MAX`.
    pub fn parse(percent: i64) -> Option<Self> {
        u8::try_from(percent)
            .ok()
            .filter(|p| (Self::MIN..=Self::MAX).contains(p))
            .map(Self)
    }

    pub const fn percent(self) -> u8 {
        self.0
    }

    /// The share of acts that must prove right, 0.5 to 0.99.
    pub fn share(self) -> f64 {
        f64::from(self.0) / 100.0
    }
}

/// The duels' agreement share until the coach sets one.
const DEFAULT_DUEL_AGREEMENT: f64 = 0.7;

/// The permission prompts' time limit until the owner sets one (the owner's choice, 2026-11-04).
const DEFAULT_PERMISSION_WAIT: Duration = Duration::minutes(30);

impl Settings {
    /// The least share of a duel judge's picks the owner leaves standing for duels of a kind of work to go on.
    pub fn duel_agreement(&self) -> f64 {
        match self.duel_agreement_percent.as_ref().map(|s| s.value) {
            None => DEFAULT_DUEL_AGREEMENT,
            #[expect(
                clippy::as_conversions,
                reason = "no From<i64> for f64; a percent is a small whole number, which a float holds exactly, and \
                          rounding a larger one to the nearest float is the reading wanted"
            )]
            Some(percent) => percent as f64 / 100.0,
        }
    }

    /// How long a permission prompt waits for the lead or the owner before its driver decides.
    pub fn permission_wait(&self) -> Duration {
        match self.permission_wait_minutes.as_ref().map(|s| s.value) {
            Some(minutes) if minutes > 0 => Duration::minutes(minutes),
            _ => DEFAULT_PERMISSION_WAIT,
        }
    }
}
