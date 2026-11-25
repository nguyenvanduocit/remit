use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Time, text_enum};

/// One answer a decider gave to one typed question about one state (ADR-0118), with what the code made of it and, once
/// known, whether it proved right. A question asked of a state is an asking: the acting decider's judgment and its
/// shadows' share it, so the outcome recorded on the asking scores every one of them on the same event. Judgments grow
/// without bound, so each is a record of its own; the state itself is never kept, only its digest.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Judgment {
    pub id: String,
    /// The asking it answers: one question about one state, shared by the acting judgment and its shadows.
    pub asking: String,
    /// The question's id and version, `memory.same_claim@1`.
    pub question: String,
    pub subject: JudgmentSubject,
    /// The SHA-256 of the state the question was asked about, in hex.
    pub state_digest: String,
    /// The pinned model id it was asked of: what its standing is kept under.
    pub decider: String,
    /// The model that answered, as the provider named it (the dated build behind the pinned id).
    pub model: String,
    pub answer: JudgmentAnswer,
    /// What the asking picked (the acting decider's most probable answer): the event its outcome says was right or wrong.
    pub pick: JudgmentPick,
    /// This answer's probability of the pick: what its Brier score and AUC are measured on.
    pub p: f64,
    /// Its share of the call's cost, in US dollars.
    pub cost_usd: f64,
    /// How long its call took, in milliseconds.
    pub latency_ms: i64,
    pub used: JudgmentUse,
    /// The probability at or above which the acting answer acted alone, or spoke where its question speaks only when sure,
    /// when it was asked: the bar it was held to, kept with it so it reads back as it was and not as it is now. None: no
    /// bar applied (a shadow, a question that always informs, or a target precision no probability reached).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    /// Whether the pick proved right, once known; the same on every judgment of the asking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<JudgmentOutcome>,
    pub created_at: Time,
}

/// What a judgment is about.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct JudgmentSubject {
    pub kind: SubjectKind,
    /// Its id; a memory pair's is the two memories' ids joined by `+`.
    pub id: String,
    /// The chain it was asked in, where the owner opens it; empty when it belongs to none.
    pub home: String,
}

impl JudgmentSubject {
    /// The report `report` of card `card`, asked in the chain `home`: its id joins theirs with `#` (`report_of` reads it).
    pub fn report(card: &str, report: &str, home: &str) -> Self {
        Self {
            kind: SubjectKind::Report,
            id: format!("{card}#{report}"),
            home: home.to_owned(),
        }
    }

    /// The card and the report of a report subject, in that order; none for any other subject.
    pub fn report_of(&self) -> Option<(&str, &str)> {
        match self.kind {
            SubjectKind::Report => self.id.split_once('#'),
            _ => None,
        }
    }

    /// The pair of memories `new` and `old`, asked in the chain `home`: its id joins theirs with `+` (`pair` reads it).
    pub fn memory_pair(new: &str, old: &str, home: &str) -> Self {
        Self {
            kind: SubjectKind::MemoryPair,
            id: format!("{new}+{old}"),
            home: home.to_owned(),
        }
    }

    /// The two memories of a memory pair, the newer first; none for any other subject.
    pub fn pair(&self) -> Option<(&str, &str)> {
        match self.kind {
            SubjectKind::MemoryPair => self.id.split_once('+'),
            _ => None,
        }
    }
}

text_enum! {
    /// What kind of thing a judgment is about.
    #[derive(Default)]
    pub enum SubjectKind {
        #[default]
        Card = "card",
        MemoryPair = "memory_pair",
        Reading = "reading",
        InboxItem = "inbox_item",
        Change = "change",
        /// A report a card sent its Chair: what the watchdog's answers are about, so the verdict on it proves them and no
        /// other (ADR-0118, point 6). Its id is the card's and the report's: `JudgmentSubject::report` joins them.
        Report = "report",
    }
}

/// A decider's answer, typed by the kind of its question.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", rename_all = "snake_case")]
pub enum JudgmentAnswer {
    /// The probability the answer to a yes-or-no question is yes.
    Noul { p: f64 },
    /// The option chosen, with the probability of each option and the decider's confidence.
    Choice {
        choice: String,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
    },
    /// The expected level on the rubric, with the probability of each level, in their order, and the confidence.
    Score {
        score: f64,
        probabilities: Vec<f64>,
        confidence: f64,
    },
}

crate::json_tagged!(JudgmentAnswer, "type");

impl Default for JudgmentAnswer {
    fn default() -> Self {
        Self::Noul { p: 0.0 }
    }
}

/// What an answer picked, typed by the kind of its question: the event its outcome says was right or wrong.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(remote = "Self", rename_all = "snake_case")]
pub enum JudgmentPick {
    /// Yes to a yes-or-no question.
    Yes,
    #[default]
    No,
    /// The option of a choice, by its name.
    Option { name: String },
    /// The level of a rubric, by its index (the lowest `0`) and its words.
    Level { index: u32, name: String },
}

crate::json_tagged!(JudgmentPick, "type");

text_enum! {
    /// What the code made of a judgment.
    #[derive(Default)]
    pub enum JudgmentUse {
        /// It decided alone: its probability reached the act threshold.
        Acted = "acted",
        /// It was unsure, so the point's System Two (an agent's turn, or the rule) decided.
        Escalated = "escalated",
        /// It was shown to the agent that decides, which may follow it or not.
        Informed = "informed",
        /// A question that speaks only when sure kept it to itself: recorded and scored, and no agent was told.
        Held = "held",
        /// A decider other than the acting one, asked only to be measured: nothing acts on it.
        #[default]
        Shadow = "shadow",
    }
}

/// Whether a judgment's pick proved right, who said so and when.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct JudgmentOutcome {
    pub proved: Proved,
    pub source: OutcomeSource,
    pub at: Time,
}

text_enum! {
    /// What a judgment's pick proved.
    #[derive(Default)]
    pub enum Proved {
        Right = "right",
        Wrong = "wrong",
        /// Its outcome can no longer be known (its subject went before it showed).
        #[default]
        Unknown = "unknown",
    }
}

text_enum! {
    /// What told whether a judgment's pick proved right (ADR-0118, each point's outcome).
    #[derive(Default)]
    pub enum OutcomeSource {
        /// The librarian named a memory pair: supersedes or links, or `distinct_from`.
        #[default]
        Librarian = "librarian",
        /// A landing held, or was reverted or breached.
        Gate = "gate",
        /// The owner overturned the decision.
        Owner = "owner",
        /// The Assistant's second opinion differed.
        SecondOpinion = "second_opinion",
        /// A second opinion on an answer that parted from a decider: shown, but not a sample of the decider's answers, so
        /// no standing is computed from it.
        SecondOpinionTargeted = "second_opinion_targeted",
        /// The card was accepted, or climbed a tier.
        Card = "card",
        /// The chain a reading started did work, or finished with nothing to do.
        Chain = "chain",
    }
}
