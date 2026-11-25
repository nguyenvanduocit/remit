use serde::{Deserialize, Serialize};

use crate::Time;
use crate::text_enum;

text_enum! {
    /// Whether a mentor can be brought onto a card.
    #[derive(Default)]
    pub enum MentorStatus {
        /// The Chair may bring them onto a card.
        #[default]
        Active = "active",
        /// The owner took them off the pool: no help is offered them, their codes stop.
        Removed = "removed",
    }
}

/// A person who helps the AI employees (ADR-0062): a PO, a designer or the owner himself, described by their strengths in
/// their own words, never by a position. A mentor holds no card and no seat: the Chair brings one onto one card (Help),
/// and their messages steer that card's own session. They work from the codes the owner had made for them (WorkerKey).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Mentor {
    pub id: String,
    /// Their name, as the owner gave it.
    pub label: String,
    /// What they are good at, in their own or the owner's words.
    pub strengths: String,
    pub status: MentorStatus,
    /// What they last said of when they can help; none until they say it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability: Option<Availability>,
    /// Counts its stored changes: of two copies, a channel keeps the one with the higher.
    pub rev: i64,
    pub created_at: Time,
}

/// What a mentor says of when they can help ("away Friday", "full this week"), and when they said it. A Chair reads it
/// before it brings them onto a card.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Availability {
    pub note: String,
    pub at: Time,
}

text_enum! {
    /// Where a mentor's help on a card stands.
    #[derive(Default)]
    pub enum HelpState {
        /// The Chair asked; the mentor has not answered.
        #[default]
        Offered = "offered",
        /// The mentor took it on: their messages steer the card's session.
        Accepted = "accepted",
        /// The mentor said no, for `why`.
        Declined = "declined",
        /// The card ended, or the Chair ended the help, for `why`.
        Ended = "ended",
    }
}

/// One mentor on one card, for one problem (ADR-0062): the Chair offers it, the mentor accepts or declines, and it ends
/// with the card. The card, its report and its record stay the AI employee's; the help is the record of what the person
/// gave.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Help {
    pub id: String,
    pub chain: String,
    pub card: String,
    pub mentor: String,
    /// What the AI is stuck on, in the Chair's words.
    pub problem: String,
    pub state: HelpState,
    /// The mentor's reason for declining, or why the help ended.
    pub why: String,
    pub offered_at: Time,
    /// When an offer not answered by then is told to the Chair; it is told once.
    pub deadline: Time,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answered_at: Option<Time>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<Time>,
    /// Counts its stored changes: of two copies, a channel keeps the one with the higher.
    pub rev: i64,
}

impl Help {
    /// Whether the help still stands: offered or accepted.
    pub fn open(&self) -> bool {
        matches!(self.state, HelpState::Offered | HelpState::Accepted)
    }
}
