//! What kind of entry an event of a card's stream is, and where it stands: each kind has its own set of statuses
//! (ADR-0107 decision 5). The store keeps the kind and the status as two texts and reads them together, once, with
//! [`EventStatus::read`].

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::text_enum;
use crate::{EventKind, UnknownValue};

text_enum! {
    /// Where a message sent to a card stands in its queue (the outbox).
    pub enum PromptStatus {
        /// Waiting for the card's session to take it.
        Queued = "queued",
        /// Taken by the session for a turn of its own.
        Delivered = "delivered",
        /// Put into the turn the card was already in.
        Steered = "steered",
        /// Never read: the session it waited for ended for good.
        Dropped = "dropped",
    }
}

text_enum! {
    /// The status of a tool call.
    pub enum ToolStatus {
        Pending = "pending",
        InProgress = "in_progress",
        Completed = "completed",
        Failed = "failed",
    }
}

impl ToolStatus {
    /// Says whether the call has ended, done or failed.
    pub const fn ended(self) -> bool {
        matches!(self, Self::Completed | Self::Failed)
    }
}

text_enum! {
    /// How a turn ended that was not an error.
    pub enum StopReason {
        EndTurn = "end_turn",
        Cancelled = "cancelled",
    }
}

text_enum! {
    /// Where work the server runs for a card between its turns (a submit) stands.
    pub enum JobStatus {
        Running = "running",
        Completed = "completed",
        Failed = "failed",
    }
}

text_enum! {
    /// Where work a session's process runs in the background stands (a background shell, a Monitor, a subagent, a
    /// workflow run): running, then ended as Claude Code says it did (its task_notification's status), or stopped when
    /// it left the process's background work without one.
    #[derive(Default)]
    pub enum TaskStatus {
        #[default]
        Running = "running",
        Completed = "completed",
        Failed = "failed",
        Stopped = "stopped",
    }
}

text_enum! {
    /// What a system event of a card's stream tells: a lifecycle note or an error, said by Remit or by Claude Code.
    pub enum SystemNote {
        /// A note in words alone.
        Plain = "",
        /// The card's next message opens a new session; one opened to recover a lost one stays so until a turn has read
        /// its lead to the end.
        NewSession = "new_session",
        /// A recovered session's lead was read to the end by a turn.
        ContextRestored = "context_restored",
        /// A tool call waits for a permission answer.
        PermissionAsked = "permission_asked",
        /// A permission prompt was answered, or a tool call refused by rule.
        PermissionAnswered = "permission_answered",
        /// What the agent marked as a block of its work (`chains::blocks`).
        Block = "block",
        /// The card's work was handed over to another agent.
        HandedOver = "handed_over",
        /// The card's session moved to another account.
        Moved = "moved",
        /// The card waits for an account to take its work.
        Waiting = "waiting",
        /// A request to the model failed and is tried again after a wait.
        Retrying = "retrying",
        /// The turn or the session failed.
        Error = "error",
        /// Claude Code compacted the conversation, or failed to.
        Compacted = "compacted",
        /// The session read a CLAUDE.md or rule file.
        InstructionsRead = "instructions_read",
        /// Claude Code changed the session's model on its own.
        ModelSwitched = "model_switched",
        /// A rail kept something out of the session.
        Refused = "refused",
    }
}

/// An event's kind with its status, one variant per kind: a status of one kind cannot stand on an event of another. Each
/// variant is named as its kind's text, which is how an event written for a channel names it (`Event`, `json_paired!`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventStatus {
    /// What the agent said, its chunks joined: no status.
    #[default]
    Message,
    /// What the agent thought, its chunks joined: no status.
    Thought,
    Tool(ToolStatus),
    Prompt(PromptStatus),
    Stop(StopReason),
    System(SystemNote),
    Job(JobStatus),
    Task(TaskStatus),
}

impl EventStatus {
    /// The status text of an event of kind as stored: the one place the two are read together.
    pub fn read(kind: EventKind, text: &str) -> Result<Self, UnknownValue> {
        Ok(match kind {
            EventKind::Message => no_status(Self::Message, text)?,
            EventKind::Thought => no_status(Self::Thought, text)?,
            EventKind::Tool => Self::Tool(text.parse()?),
            EventKind::Prompt => Self::Prompt(text.parse()?),
            EventKind::Stop => Self::Stop(text.parse()?),
            EventKind::System => Self::System(text.parse()?),
            EventKind::Job => Self::Job(text.parse()?),
            EventKind::Task => Self::Task(text.parse()?),
        })
    }

    /// The kind of event it is the status of.
    pub const fn kind(self) -> EventKind {
        match self {
            Self::Message => EventKind::Message,
            Self::Thought => EventKind::Thought,
            Self::Tool(_) => EventKind::Tool,
            Self::Prompt(_) => EventKind::Prompt,
            Self::Stop(_) => EventKind::Stop,
            Self::System(_) => EventKind::System,
            Self::Job(_) => EventKind::Job,
            Self::Task(_) => EventKind::Task,
        }
    }

    /// The status as it is stored: empty for a kind that has none.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Message | Self::Thought => "",
            Self::Tool(s) => s.as_str(),
            Self::Prompt(s) => s.as_str(),
            Self::Stop(s) => s.as_str(),
            Self::System(s) => s.as_str(),
            Self::Job(s) => s.as_str(),
            Self::Task(s) => s.as_str(),
        }
    }
}

impl fmt::Display for EventStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The status of a kind that has none: the empty text, and no other.
fn no_status(status: EventStatus, text: &str) -> Result<EventStatus, UnknownValue> {
    if text.is_empty() {
        return Ok(status);
    }
    Err(UnknownValue {
        name: "EventStatus",
        text: text.to_owned(),
        values: &[""],
    })
}
