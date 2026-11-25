//! Remit's decision layer (ADR-0118), pure: the typed questions Remit asks, each declared once (`QUESTIONS`) and asked
//! as a checked `Question`, what a decider's answer picks and how sure it is of the pick (`pick`, `chance`), how well
//! each decider has answered a question once outcomes are known (`Standing`: Brier score and AUC), the order deciders
//! are asked in and from which probability an answer acts alone (`ranked`, `act_threshold`), and the port a decider is
//! asked through (`Decider`). It does no I/O: the store keeps the judgments (`model::Judgment`), chains asks and records
//! them, and the server fills the port with OpenRouter's Decisions API.

mod answer;
mod decider;
mod question;
mod standing;

pub use answer::{chance, pick, state_digest, use_of};
pub use decider::{DEFAULT_DECIDER, Decided, Decider, DeciderError, SHADOWS, decider_name};
pub use question::{
    AGENT_ASK, Acts, ActsOn, CARD_OFF_BRIEF, CARD_PROGRESS, ChoiceOption, Definition, Floor, Form, GATE_RISK,
    GATE_TOUCHES_RAIL, HIRE_EFFORT, HIRE_TIER_CAN, INBOX_OPTION, INBOX_REVERSIBLE, Invalid, Kind, MEMORY_CONTRADICTS,
    MEMORY_SAME_CLAIM, MayLeave, QUESTIONS, Question, SIGNAL_SCREEN, Shape, Speaks, definition, pick_words,
};
pub use standing::{
    MIN_OUTCOMES, PROVISIONAL_THRESHOLD, Scored, Standing, Threshold, act_threshold, auc, brier, ranked, shadows,
};
