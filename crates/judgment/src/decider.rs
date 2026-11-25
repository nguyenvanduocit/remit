use async_trait::async_trait;
use model::JudgmentAnswer;

use crate::question::Question;

/// The decider every question is asked of until its own outcomes say another answers it better: the owner's choice where
/// the measurement on his memory pairs ties (ADR-0118, 2026-11-19: with criteria on the question, AUC 0.994 against
/// `liquid/d1.2-ultra`'s 1.000). A pinned id, never a `~latest` alias: a standing belongs to the model that earned it.
pub const DEFAULT_DECIDER: &str = "typesafe/jev-1.14";

/// The deciders asked in shadow beside the acting one, while they are not it (`shadows`): the best measured for ADR-0118
/// and the free one, so a better decider shows by its numbers. Pinned ids.
pub const SHADOWS: [&str; 2] = ["liquid/d1.2-ultra", "perplexity/pplx-decider-v1.3:free"];

/// The deciders Remit knows by name, as the owner reads them: each pinned id once, with its name.
const NAMES: [(&str, &str); 3] = [
    (DEFAULT_DECIDER, "Jev 1.14"),
    ("liquid/d1.2-ultra", "Liquid D1.2 Ultra"),
    ("perplexity/pplx-decider-v1.3:free", "Perplexity Decider (free)"),
];

/// decider's name as the owner reads it; its pinned id when Remit knows it by no name.
pub fn decider_name(decider: &str) -> &str {
    NAMES
        .iter()
        .find(|(id, _)| *id == decider)
        .map_or(decider, |(_, name)| name)
}

/// What a decider gave for one call: the model that answered, its answer to each question in the questions' order, and
/// what the call cost.
#[derive(Debug, Clone, PartialEq)]
pub struct Decided {
    /// The model as the provider named it: the dated build behind the pinned id asked.
    pub model: String,
    /// None for a question it gave no answer to: it refused it, left it out, or answered in a shape the API does not
    /// document. That question is not answered; the others are.
    pub answers: Vec<Option<JudgmentAnswer>>,
    /// In US dollars, for the whole call.
    pub cost_usd: f64,
}

/// Why a decider gave nothing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DeciderError {
    /// No key is configured: every point runs its System Two, and nothing is wrong.
    #[error("no key is configured")]
    NoKey,
    /// Anything else: the network, the provider, an answer that is no answer at all.
    #[error("{0}")]
    Failed(String),
}

/// The deciders: models that read a state and typed questions and give each a calibrated answer with no text (OpenRouter's
/// Decisions API in the server). Declared here, by the layer that asks them.
#[async_trait]
pub trait Decider: Send + Sync {
    /// decider's answers to questions about state, in the questions' order.
    async fn decide(
        &self,
        decider: &str,
        state: &serde_json::Value,
        questions: &[Question],
    ) -> Result<Decided, DeciderError>;
}
