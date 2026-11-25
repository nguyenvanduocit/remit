//! Why a session closed, or the turn it was in was cut short, and how what it left running ends: a background task's
//! stopped event keeps its cut in its data (`cut`), beside the words its text says it in (ADR-0107, *Typed values*).

use serde::{Deserialize, Serialize};

use crate::{Event, EventStatus, ForeignJson, TaskStatus};

/// Why a session closed, or the turn it was in was cut short: what decides how the turn's end is read (a turn a pause cut
/// goes on once the chain resumes), and the words the tool calls, tasks and runs it left open end with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(remote = "Self", rename_all = "snake_case")]
pub enum Cut {
    /// The chain was paused: a turn it cut goes on once the chain resumes.
    Paused,
    /// The chain went to wait for its check-back: its agents' processes end, and with them what they ran in the
    /// background; the tasks' events say it, and the next wake tells the Chair.
    Slept,
    /// The chain finished.
    ChainFinished,
    /// The chain was deleted.
    ChainDeleted,
    /// The server restarted: the process that did the work died.
    ServerRestarted,
    /// The AI runtime restarted: the process that held the session died, the server did not.
    RuntimeRestarted,
    /// The server stopped: its next start resumes the card.
    ServerStopped,
    /// Someone (the owner, or an outside agent) interrupted the turn.
    Interrupted { who: String },
    /// The agent was stopped for good.
    AgentStopped,
    /// The workflow the agent worked in was stopped.
    WorkflowStopped,
    /// The agent sent nothing for too long (the liveness rail).
    SilentTurn,
    /// The silent turn did not end when cut, so its session closed.
    SilentSession,
    /// Told to end its turn because it kept repeating itself, the agent kept calling tools (the repetition rail).
    Repeating,
    /// The agent's work was handed over to another card.
    HandedOver,
    /// The agent's account is out, and it waits for one.
    WaitsForAccount,
    /// The agent's session failed.
    SessionFailed,
    /// The goal check the agent judged ended.
    GoalCheckEnded,
    /// The duel the agent judged was judged.
    DuelJudged,
    /// The round of attempts the agent judged was picked in.
    RoundJudged,
    /// The second opinion the agent gave was recorded.
    SecondOpinionGiven,
    /// The owner started the Assistant over.
    StartedOver,
    /// The session's context is full.
    ContextFull,
    /// The subagent's task ended, as status says.
    SubagentEnded { status: TaskStatus },
    /// The chain was given a capability: the session opens again with it.
    CapabilityGiven { giver: String, capability: String },
}

crate::json_tagged!(Cut, "why");

impl std::fmt::Display for Cut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Paused => f.write_str("The chain was paused"),
            Self::Slept => f.write_str("The chain is waiting: its sessions closed"),
            Self::ChainFinished => f.write_str("The chain finished"),
            Self::ChainDeleted => f.write_str("The chain was deleted"),
            Self::ServerRestarted => f.write_str("The server restarted"),
            Self::RuntimeRestarted => f.write_str("The AI runtime restarted"),
            Self::ServerStopped => f.write_str("The server stopped"),
            Self::Interrupted { who } => write!(f, "{who}{INTERRUPTED}"),
            Self::AgentStopped => f.write_str("The agent was stopped"),
            Self::WorkflowStopped => f.write_str("The workflow was stopped"),
            Self::SilentTurn => f.write_str("Remit cut the silent turn short"),
            Self::SilentSession => f.write_str("Remit closed the session of a silent turn"),
            Self::Repeating => f.write_str("Remit ended the turn: the agent kept repeating the same calls"),
            Self::HandedOver => f.write_str("The work was handed over"),
            Self::WaitsForAccount => f.write_str("The agent waits for an account"),
            Self::SessionFailed => f.write_str("The agent's session failed"),
            Self::GoalCheckEnded => f.write_str("The goal check ended"),
            Self::DuelJudged => f.write_str("The duel was judged"),
            Self::RoundJudged => f.write_str("The round's attempt was picked"),
            Self::SecondOpinionGiven => f.write_str("The second opinion was given"),
            Self::StartedOver => f.write_str("The owner started over"),
            Self::ContextFull => f.write_str("Its context is full"),
            Self::SubagentEnded { status } => write!(f, "{SUBAGENT}{status}"),
            Self::CapabilityGiven { giver, capability } => {
                write!(f, "{giver}{GAVE}{capability}{OPENS_AGAIN}")
            }
        }
    }
}

const INTERRUPTED: &str = " interrupted the turn";
const SUBAGENT: &str = "The subagent ";
const GAVE: &str = " gave the chain the ";
const OPENS_AGAIN: &str = " capability: this session opens again with it";
const BEFORE_IT_FINISHED: &str = " before it finished.";

/// The key of a stopped task event's data that holds its cut.
const CUT_KEY: &str = "cut";

/// The cuts that hold nothing, as an event written before the cut was kept in its data (migration 130) can name them.
/// That set is closed: a cut added since is written in the data, never read back from words.
const UNIT_CUTS: [Cut; 21] = [
    Cut::Paused,
    Cut::Slept,
    Cut::ChainFinished,
    Cut::ChainDeleted,
    Cut::ServerRestarted,
    Cut::RuntimeRestarted,
    Cut::ServerStopped,
    Cut::AgentStopped,
    Cut::WorkflowStopped,
    Cut::SilentTurn,
    Cut::SilentSession,
    Cut::Repeating,
    Cut::HandedOver,
    Cut::WaitsForAccount,
    Cut::SessionFailed,
    Cut::GoalCheckEnded,
    Cut::DuelJudged,
    Cut::RoundJudged,
    Cut::SecondOpinionGiven,
    Cut::StartedOver,
    Cut::ContextFull,
];

/// The text a tool call, task or job a stop cut off ends with, why being what stopped it.
pub fn cut_short(why: &str) -> String {
    format!("{why}{BEFORE_IT_FINISHED}")
}

impl Cut {
    /// The cut a stopped task event's data holds; none for any other.
    pub fn of(data: Option<&ForeignJson>) -> Option<Cut> {
        <Cut as Deserialize>::deserialize(data?.field(CUT_KEY)?).ok()
    }

    /// data with this cut in it, beside what it holds.
    pub fn into_data(&self, data: Option<&ForeignJson>) -> ForeignJson {
        let cut = serde_json::to_value(self).unwrap_or_default(); // a Cut always writes as JSON
        ForeignJson::with_field(data, CUT_KEY, cut)
    }

    /// Stops background task event task, which this cut ended: stopped, its words saying why, its data holding the cut.
    pub fn stop_task(&self, task: &mut Event) {
        task.status = EventStatus::Task(TaskStatus::Stopped);
        task.text = cut_short(&self.to_string());
        task.data = Some(self.into_data(task.data.as_ref()));
    }

    /// The cut the text of a task stopped before its cut was kept in its data says (`cut_short` of its words); none for
    /// any other text. Only the store's event reader calls it, on an event written in that older shape.
    pub fn read_from_words(text: &str) -> Option<Cut> {
        let why = text.strip_suffix(BEFORE_IT_FINISHED)?;
        if let Some(unit) = UNIT_CUTS.iter().find(|c| c.to_string() == why) {
            return Some(unit.clone());
        }
        if let Some(who) = why.strip_suffix(INTERRUPTED) {
            return Some(Cut::Interrupted { who: who.to_owned() });
        }
        if let Some(status) = why.strip_prefix(SUBAGENT) {
            return status.parse().ok().map(|status| Cut::SubagentEnded { status });
        }
        let (giver, capability) = why.strip_suffix(OPENS_AGAIN)?.split_once(GAVE)?;
        Some(Cut::CapabilityGiven {
            giver: giver.to_owned(),
            capability: capability.to_owned(),
        })
    }
}
