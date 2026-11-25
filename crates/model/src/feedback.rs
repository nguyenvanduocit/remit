use serde::{Deserialize, Serialize};

use crate::Time;
use crate::text_enum;

text_enum! {
    /// What an agent's feedback is about.
    #[derive(Default)]
    pub enum FeedbackKind {
        /// How it is asked to work: a brief, a rule, a tool, the way the work is handed to it.
        #[default]
        Practice = "practice",
        /// What got in its way.
        Difficulty = "difficulty",
        /// What it finds wrong or unfair.
        Complaint = "complaint",
        /// What it would change, and how.
        Suggestion = "suggestion",
    }
}

/// An agent telling the organisation how it works: how it is asked to work, what got in its way, what it complains of,
/// what it would change. It is the agent's own words, which the owner reads in the Feedback tab; nothing acts on it by
/// itself.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Feedback {
    pub id: String,
    pub kind: FeedbackKind,
    pub text: String,
    /// `home`, `card_id`, `agent`, `name`, `model` and `effort` say who sent it and where it worked: its chain (or the
    /// Assistant's or a placement's home), its card, the agent definition it runs and the model and effort it ran on, as
    /// they were when it wrote.
    pub home: String,
    pub card_id: String,
    pub agent: String,
    pub name: String,
    pub model: String,
    pub effort: String,
    /// When the owner opened it; none while it waits to be read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_at: Option<Time>,
    pub created_at: Time,
    /// Counts its stored changes: of two copies, a channel keeps the one with the higher.
    pub rev: i64,
}
