//! A replay runs the candidate of a change item (an agent template's instructions, a skill) against goals Remit already
//! recorded with the goal judge's verdict, one chain per goal, and compares the new verdicts with the recorded ones: the
//! change is let through only when it holds (ADR-0069). It is kept on the item it measures.

use serde::{Deserialize, Serialize};

use crate::text_enum;
use crate::{Actor, Time};

text_enum! {
    /// How one goal fared under the goal judge: its latest verdict read as met or not.
    #[derive(Default)]
    pub enum ReplayOutcome {
        /// Its replay chain has not finished, or is not queued yet.
        #[default]
        Pending = "",
        /// Nothing the judge found is not_met, and nothing is unproven but a landing or deploy.
        Met = "met",
        /// The judge found it not met, or left it unproven on more than a landing or deploy.
        NotMet = "not_met",
        /// The chain finished with no verdict of the goal judge, or could not be replayed.
        Unjudged = "unjudged",
    }
}

text_enum! {
    /// Where a replay stands as a whole.
    #[derive(Default)]
    pub enum ReplayVerdict {
        /// A goal is still being replayed.
        #[default]
        Running = "running",
        /// The candidate met at least as many recorded goals as were met before.
        Held = "held",
        /// It met fewer: the change is refused.
        Regressed = "regressed",
        /// No goal was judged both times, so nothing was measured.
        Inconclusive = "inconclusive",
        /// Its item was answered or replaced before it ended.
        Stopped = "stopped",
    }
}

/// One recorded goal a replay runs again: the chain that recorded it, its title, how the goal judge ruled on it then, the
/// chain that replays it (chosen before it is queued, so a restart neither loses nor doubles it) and how it fares now.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ReplayBrief {
    pub chain: String,
    pub title: String,
    pub recorded: ReplayOutcome,
    pub replay: String,
    pub outcome: ReplayOutcome,
    /// Why a goal could not be replayed, when it could not.
    pub why: String,
}

/// What a replay's briefs come to: over the goals judged both times, how many were met then and now, which went from met
/// to not met (`regressed`) and the other way (`improved`), by recorded chain id.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ReplayScore {
    pub verdict: ReplayVerdict,
    pub judged: i64,
    pub recorded_met: i64,
    pub replay_met: i64,
    pub regressed: Vec<String>,
    pub improved: Vec<String>,
}

/// The replay of one change item's candidate: who asked for it and when, the recorded goals it runs, and their score.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Replay {
    pub by: Actor,
    pub at: Time,
    pub briefs: Vec<ReplayBrief>,
    pub score: ReplayScore,
}
