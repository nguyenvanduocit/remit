//! The outcomes report (ADR-0107 *Decision 3*): the picture of how well the agents are doing that `outcomes` computes,
//! which the owner's channel and the agents' tools both carry. Only the computation lives in `outcomes`.

use serde::{Deserialize, Serialize};

use crate::{ChainKind, Time};

/// The stretch of time a report reads, in words for whoever shows it, and the choices to read instead
/// (`outcomes::Span` lays it out).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeSpan {
    pub range: OutcomeRange,
    pub window: OutcomeWindow,
    /// The last period, in words: "the last 7 days".
    pub last: String,
    /// The period before it: "the 7 days before".
    pub before: String,
    /// One period's name, to count them back: "week".
    pub unit: String,
    /// The choices to read instead, in order. Each fits when it holds the other of this span at least twice: a range
    /// this window fits in twice, a window that fits twice in this range.
    pub ranges: Vec<OutcomeRangeChoice>,
    pub windows: Vec<OutcomeWindowChoice>,
}

/// How far back a report reads: its periods cover at least this much, ending now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OutcomeRange {
    #[serde(rename = "7d")]
    Days7,
    #[serde(rename = "30d")]
    Days30,
    #[serde(rename = "90d")]
    Days90,
    #[serde(rename = "180d")]
    Days180,
    #[serde(rename = "365d")]
    Days365,
}

/// How long one period of a report is: the last period is compared against the one before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OutcomeWindow {
    #[serde(rename = "1d")]
    Days1,
    #[serde(rename = "7d")]
    Days7,
    #[serde(rename = "30d")]
    Days30,
}

/// A range a report can read, and whether the window read now fits in it twice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeRangeChoice {
    pub key: OutcomeRange,
    pub label: String,
    pub fits: bool,
}

/// A window a report can compare, and whether it fits the range read now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeWindowChoice {
    pub key: OutcomeWindow,
    pub label: String,
    pub fits: bool,
}

/// One figure of a KPI: a share (`n` of `of`, as `value` from 0 to 1), an average over `of` events (of hours, or of a
/// count), or a count.
/// `of` is how many events the figure is over, for a share or an average; 0 for a count. `has` is false when there is
/// nothing to say yet, as when a share has nothing to divide.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct OutcomePoint {
    pub value: f64,
    pub n: i64,
    pub of: i64,
    pub has: bool,
}

/// What a KPI's figures are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutcomeKind {
    Rate,
    Hours,
    /// An average of a count over `of` events: landings per batch, reverts per confirmed result.
    Mean,
    Count,
}

/// Which way a KPI should move: up, down, or neither when more is neither good nor bad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutcomeBetter {
    Up,
    Down,
    Neither,
}

/// What the move from the period before to the last period says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutcomeReads {
    Better,
    Worse,
    Same,
    /// It changed, and more is neither good nor bad.
    Moved,
    /// A share or an average over fewer than `outcomes::MIN_EVENTS` events in a window: too few to tell.
    Few,
    /// A window has no figure: nothing to compare.
    None,
}

/// The last period against the one before: by how much (in the figure's own unit; a share's from 0 to 1) and what that
/// says.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OutcomeChange {
    pub delta: f64,
    pub reads: OutcomeReads,
}

/// One thing the owner steers by. It carries its own definition (`hint`), a figure for each period of the span
/// (`series`, oldest first), the three figures to compare (the last period, the one before it, and all time) and what
/// their move says.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutcomeKpi {
    pub key: String,
    pub label: String,
    pub hint: String,
    pub kind: OutcomeKind,
    pub better: OutcomeBetter,
    /// What an average is over (`of`), one of it: "landing", "confirmed result". Empty for a share or a count, whose `n` of
    /// `of` or period says it.
    pub over: String,
    pub series: Vec<OutcomePoint>,
    pub now: OutcomePoint,
    pub before: OutcomePoint,
    pub all: OutcomePoint,
    pub change: OutcomeChange,
}

/// Names a group of KPIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutcomeGroupKey {
    Headline,
    Quality,
    Landing,
    Reliability,
}

/// KPIs read together, in the order they are shown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutcomeGroup {
    pub key: OutcomeGroupKey,
    pub label: String,
    pub kpis: Vec<OutcomeKpi>,
}

/// What happened in one window of a span; the report's `all` counts everything. A report is judged once, on the
/// Chair's first word on it (`accepted` or `sent_back`); a chain's goal check on its first verdict; a landing when it
/// landed, went back to its Chair, or was taken back. Every count leaves out a client key's chains and the trial chains (ADR-0102), whose verdicts count only
/// as catches.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomePeriod {
    pub start: Time,
    /// Started.
    pub chains: i64,
    /// Reports the Chair accepted at once.
    pub accepted: i64,
    /// Reports the Chair sent back for rework.
    pub sent_back: i64,
    /// Chains whose first goal check fell here.
    pub goal_checked: i64,
    /// Of those, the ones it passed.
    pub goal_first_pass: i64,
    /// Goal checks that could not run.
    pub goal_not_run: i64,
    /// Chains whose last goal check fell here.
    pub goal_ruled: i64,
    /// Of those, the ones whose result it confirmed: approved, every key result met.
    pub goal_confirmed: i64,
    /// Reviewer and QC verdicts.
    pub verdicts: i64,
    /// Of those, the ones asking for changes.
    pub changes_requested: i64,
    /// Landings of a chain's change that reached their base here, and the batches they landed in.
    pub landed: i64,
    pub batches: i64,
    /// The hours those waited from ready to landed, all together.
    pub queue_hours: f64,
    /// Landings taken back here: by the forge's pipeline, a guardrail, a revert they conflicted with, or the owner.
    pub reverted: i64,
    /// Of those, the ones the owner took back, himself or through an agent on his word.
    pub overturned: i64,
    /// Versions the queue sent back to their Chair here, and of those the ones for a conflict with the base or a failed
    /// check on the combined tree.
    pub returned: i64,
    pub returned_conflict: i64,
    pub returned_checks: i64,
    pub failed_agents: i64,
    /// Chains finished because nothing moved.
    pub stalled: i64,
    /// Faults planted in trial chains that a reviewer ruled on here: its first verdict after the planting.
    pub reviewer_faults: i64,
    /// Of those, the ones it caught: the verdict asked for changes.
    pub reviewer_caught: i64,
    /// The same for QC.
    pub qc_faults: i64,
    pub qc_caught: i64,
    /// The same for the goal judge.
    pub judge_faults: i64,
    pub judge_caught: i64,
}

/// How the agents of one role (the agent their cards were hired as) did, over the same chains as `OutcomePeriod`. `accepted` and
/// `sent_back` are the Chair's first word on their reports; `approved` and `changes_requested` the verdicts their cards
/// gave.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeRole {
    pub role: String,
    pub agents: i64,
    pub failed: i64,
    pub turns: i64,
    pub accepted: i64,
    pub sent_back: i64,
    pub approved: i64,
    pub changes_requested: i64,
}

/// How the agents running one model at one effort did: `fable · high` and `fable · low` are two rows. An empty effort is
/// Claude Code's default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeModel {
    pub model: String,
    pub effort: String,
    pub agents: i64,
    pub failed: i64,
    pub turns: i64,
}

/// How the chains of one kind did (ADR-0113). `gated` is the chains whose change went to a reviewer or QC.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeChainKind {
    pub kind: ChainKind,
    pub chains: i64,
    pub goal_checked: i64,
    pub goal_first_pass: i64,
    pub gated: i64,
    /// Of those, the ones a reviewer or QC asked changes of.
    pub reworked: i64,
    /// Chains with a landing on its base, with one taken back, and with one still on its way.
    pub landed: i64,
    pub reverted: i64,
    pub landing: i64,
}

/// The whole picture at one moment, over a span. Periods run oldest first and are rolling: the last ends now, each is
/// one window of the span.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutcomeReport {
    pub at: Time,
    pub span: OutcomeSpan,
    pub periods: Vec<OutcomePeriod>,
    pub all: OutcomePeriod,
    pub groups: Vec<OutcomeGroup>,
    pub roles: Vec<OutcomeRole>,
    pub models: Vec<OutcomeModel>,
    pub kinds: Vec<OutcomeChainKind>,
    /// The built-in tools, from the calls recorded with a name.
    pub tools: Vec<OutcomeUsage>,
    /// The MCP tools, grouped by server.
    pub mcp: Vec<OutcomeUsage>,
    /// The skills loaded.
    pub skills: Vec<OutcomeUsage>,
}

/// How much the agents used one tool, one MCP tool (`server` names its MCP server) or one skill. `agents` and `chains`
/// count the distinct agents and chains that used it at least once; the Assistant's own agent counts in `agents` only.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutcomeUsage {
    /// Empty for a built-in tool or a skill.
    pub server: String,
    pub name: String,
    pub calls: i64,
    pub failed: i64,
    pub agents: i64,
    pub chains: i64,
}
