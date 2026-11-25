//! A run of a script in a home and the calls it made (ADR-0073). A clone shares no memory with the original.

use serde::{Deserialize, Serialize};

use crate::text_enum;
use crate::{ForeignJson, Time};

/// One run of a script in a home (ADR-0073): Claude Code's Workflow tool inside one card's process (engine claude), or a
/// script Remit replays from the journal of its steps (engine remit). Its steps are records of their own, filled in by the
/// store whenever a run is read and never stored in its document: a run grows to a thousand steps.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Workflow {
    pub id: String,
    /// Where it runs: its chain's id, or the Assistant's for a run of the Assistant's card.
    pub home: String,
    /// Whose hands it is: the card that called Claude Code's Workflow, or the Chair that called run_workflow.
    pub card: String,
    pub engine: WorkflowEngine,
    pub name: String,
    /// Its text, fixed for the run.
    pub script: String,
    /// What the script reads as args.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<ForeignJson>,
    pub status: WorkflowStatus,
    /// The phase the script named last.
    pub phase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<ForeignJson>,
    /// Why it failed or was stopped.
    pub error: String,
    /// The run whose recorded steps it reuses up to the first that differs.
    pub resumed_from: String,
    /// Engine claude only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude: Option<ClaudeRun>,
    /// The calls the script made, by seq: filled in by the store, never stored in the run's document.
    pub steps: Vec<WorkflowStep>,
    pub created_at: Time,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<Time>,
    /// Counts its stored changes, a step's included: of two copies, a channel keeps the one with the higher.
    pub rev: i64,
}

text_enum! {
    /// Who runs a script.
    #[derive(Default)]
    pub enum WorkflowEngine {
        /// Claude Code, inside the process of the card that called it; dies with it.
        #[default]
        Claude = "claude",
        /// Remit, from its journal: it waits, and survives a restart.
        Remit = "remit",
    }
}

text_enum! {
    /// Where a run stands.
    #[derive(Default)]
    pub enum WorkflowStatus {
        /// A call it made is at work.
        #[default]
        Running = "running",
        /// It waits only on a time, a signal or the owner.
        Waiting = "waiting",
        /// Its chain is paused: no call is issued.
        Paused = "paused",
        Done = "done",
        Failed = "failed",
        Stopped = "stopped",
    }
}

impl WorkflowStatus {
    /// Whether the status is an end: done, failed or stopped.
    pub fn ended(&self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Stopped)
    }
}

/// What Claude Code's launch answer named a run of engine claude by: Claude Code owns its journal, in `transcript_dir`, and
/// resumes it with `Workflow({scriptPath, resumeFromRunId})`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClaudeRun {
    pub task_id: String,
    pub run_id: String,
    pub script_path: String,
    pub transcript_dir: String,
}

/// One call a run's script made, journaled by its order: on a replay a recorded call whose verb or input hash differs fails
/// the run. The card it put to work is an ordinary card, which knows nothing of the step.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorkflowStep {
    pub workflow: String,
    pub seq: i64,
    pub verb: WorkflowVerb,
    pub label: String,
    pub phase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<ForeignJson>,
    /// Of `input`, compared on replay.
    pub hash: String,
    pub status: StepStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<ForeignJson>,
    /// Why it failed.
    pub error: String,
    /// The card the step put to work.
    pub card: String,
    /// The order its result came in among the run's (1 the first, 0 none yet): a replay hands the script its results in
    /// that order, which numbers the calls of a parallel or a pipeline as the first evaluation did.
    pub arrived: i64,
    /// Its card was sent back once for a report its schema refused: a second such report fails the call.
    pub sent_back: bool,
    pub at: Time,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<Time>,
}

text_enum! {
    /// What a step asked for: Claude Code's agent, log and phase, and Remit's own verbs, each the Chair's action.
    #[derive(Default)]
    pub enum WorkflowVerb {
        /// Put an employee to work on a prompt and return its report, accepted.
        #[default]
        Agent = "agent",
        /// Hire as hire_agent does; the card awaits accept or send_back.
        Hire = "hire",
        /// Wait for a time or a signal, as subscribe does.
        Wait = "wait",
        /// Ask the owner, as ask_owner does.
        AskOwner = "ask_owner",
        /// Accept a card's work.
        Accept = "accept",
        /// Send a card's work back with feedback.
        SendBack = "send_back",
        Log = "log",
        Phase = "phase",
        /// Run another script as one step; Remit keeps no named scripts, so the step fails saying so.
        Workflow = "workflow",
    }
}

text_enum! {
    /// Where a step stands.
    #[derive(Default)]
    pub enum StepStatus {
        #[default]
        Pending = "pending",
        Done = "done",
        Failed = "failed",
    }
}

/// The step of a run a firing resumes (`ActionKind::Workflow`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorkflowRef {
    pub run: String,
    pub seq: i64,
}
