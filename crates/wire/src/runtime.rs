//! The contract between the Remit core and its AI runtime (`remit runtime`), the child process that holds every Claude Code
//! session, the accounts' config dirs and their logins, from `web/src/rpc/runtime/define.ts`. It runs both ways: `to_core`
//! is what the runtime calls on the core, `to_runtime` what the core calls on the runtime.
//!
//! - Two calls for every wait: a call answers within its timeout, so what waits on a person or a model is a call that
//!   returns at once and a later call or entry that reports the end.
//! - A replay acts once by the id the caller minted (`sessionId`, `promptId`, `steerId`, `requestId`, `loginId`), so these
//!   calls are `idempotent` rather than taking a `Request`.
//! - `opened` ends a session's open; `exit` is the last entry of a session that opened.
//!
//! What a session reports (`ToolCall`, `Note`, `Task`, `Limit`, `Failure`, …) is declared here once: the session port
//! (`agent::Handler`, `agent::Session`) speaks these types on both sides of the link, and what the core keeps of a session
//! (`model::SessionInit`, `model::SessionContext`, `model::PlanEntry`) is `model`'s (ADR-0107 decision 4).

use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

use model::{Prompt, SessionContext, SessionInit, StopReason, SystemNote, TaskStatus, Time, ToolStatus, text_enum};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::Ack;

/// The contract's number. Additive changes keep it; any other change bumps it, and the core refuses a runtime that says
/// another number.
pub const RUNTIME_CONTRACT: i64 = 5;

/// How long the runtime gives Claude to start a session before it answers the open with a `Service` failure. The core
/// waits longer than this for that answer, so the runtime's verdict, not the core's clock, says why an open failed.
pub const SESSION_OPEN_WAIT: Duration = Duration::from_secs(180);

text_enum! {
    /// What part of the provider failed a turn.
    pub enum FailureCategory {
        /// The account cannot be used (it may not use the model, it is signed out).
        Access = "access",
        /// A rate or usage limit.
        Limit = "limit",
        /// The provider's service is down or overloaded.
        Service = "service",
    }
}

/// A turn that ended because of the provider behind the agent: its account, its limits or its service. It is what the
/// turn's error is when the provider is at fault, so a caller can have the work wait for the provider rather than for the
/// owner. `title` is what the provider said; `resets_at` when the limit it hit resets, none when it did not say.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub category: FailureCategory,
    pub title: String,
    pub resets_at: Option<Time>,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.title)
    }
}

impl std::error::Error for Failure {}

text_enum! {
    /// Whether the account's limit still takes requests.
    pub enum LimitStatus {
        /// It takes them.
        Allowed = "allowed",
        /// It takes them, and the provider warns the limit is near.
        Warning = "warning",
        /// It takes none until it resets.
        Rejected = "rejected",
    }
}

/// What the provider said of the account's limit at a turn (Claude Code's rate_limit_event), each time it said it:
/// `window` is the limit it speaks of as it names it (`five_hour`, `seven_day`, `seven_day_opus`, …; empty when it did not
/// say), `utilization` how much of it is used from 0 to 1 and `resets_at` when it resets; none when the provider did not
/// say.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Limit {
    pub status: LimitStatus,
    pub window: String,
    pub utilization: Option<f64>,
    pub resets_at: Option<Time>,
}

text_enum! {
    /// The kind of a tool call, as the stream records it (`model::Event::tool_kind`) and the page groups it.
    pub enum ToolKind {
        Read = "read",
        Edit = "edit",
        Delete = "delete",
        Move = "move",
        Search = "search",
        Execute = "execute",
        Think = "think",
        Fetch = "fetch",
        SwitchMode = "switch_mode",
        Other = "other",
    }
}

/// One use of a tool by the agent, as it grows over entries (the same `id` is the same call): Claude Code's tool name
/// (Bash, Edit, mcp__remit__update_plan, …) and input, and what they read as (`kind`, `title`). `output` is the text the
/// tool answered with, once it has. The input is JSON Claude Code defines, carried as `model::ForeignJson` is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub kind: ToolKind,
    pub title: String,
    pub status: ToolStatus,
    #[serde(with = "model::stored_json::foreign_object")]
    pub input: Map<String, Value>,
    pub output: String,
}

text_enum! {
    /// Which of the things Claude Code does to a session a [`Note`] tells.
    pub enum NoteKind {
        /// A request to the model failed and Claude Code tries it again after a wait.
        Retrying = "retrying",
        /// Claude Code compacted the conversation: what the agent was told before is a summary now.
        Compacted = "compacted",
        /// Claude Code tried to compact the conversation and could not: it holds what it held.
        CompactFailed = "compact_failed",
        /// The session read a CLAUDE.md or rule file.
        InstructionsRead = "instructions_read",
        /// Claude Code changed the session's model on its own.
        ModelSwitched = "model_switched",
        /// A rail kept something out of the session: a tool call while its turn is being cancelled, a settings change.
        Refused = "refused",
    }
}

/// Something Claude Code did to a session beside the agent's work, which the owner follows the agent by: `kind` which it
/// is, `title` what it is about when it is about one thing (the instructions file read; empty otherwise), `text` says it
/// in words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub kind: NoteKind,
    pub title: String,
    pub text: String,
}

impl From<NoteKind> for SystemNote {
    /// The system event a note is recorded as.
    fn from(kind: NoteKind) -> Self {
        match kind {
            NoteKind::Retrying => SystemNote::Retrying,
            NoteKind::Compacted | NoteKind::CompactFailed => SystemNote::Compacted,
            NoteKind::InstructionsRead => SystemNote::InstructionsRead,
            NoteKind::ModelSwitched => SystemNote::ModelSwitched,
            NoteKind::Refused => SystemNote::Refused,
        }
    }
}

impl Note {
    /// A note about nothing in particular: its words alone.
    pub fn says(kind: NoteKind, text: String) -> Self {
        Note {
            kind,
            title: String::new(),
            text,
        }
    }
}

/// How full the session's context is: `used` of `size` tokens.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub used: u64,
    pub size: u64,
}

/// The task type of a subagent.
pub const TASK_SUBAGENT: &str = "local_agent";

/// The task type of a run of Claude Code's Workflow tool.
pub const TASK_WORKFLOW: &str = "local_workflow";

/// Work a session's process runs beside the turn that started it: a shell command run in the background, a Monitor
/// watching one, a subagent, a workflow run. While it runs, the agent can be told of it (a Monitor's events, the task's
/// end) and take a turn of its own on that news. A subagent or a workflow run is reported from its start, whether or not
/// it runs in the background; the session's other tasks only once they are in the background.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    /// The process's name for it; a subagent's is its agent id, the one `agent::Handler::on_subagent` is asked for.
    pub id: String,
    /// Claude Code's task type: `TASK_SUBAGENT`, `TASK_WORKFLOW`, local_bash (a background shell or a Monitor), ...
    pub kind: String,
    pub title: String,
    /// The tool call that started it, once known.
    pub tool_call_id: String,
    pub output_file: String,
    pub status: TaskStatus,
    /// How it ended, in Claude Code's words.
    pub summary: String,
    /// A subagent's and a workflow run's, from their start: the subagent type it runs as, what it was asked (a workflow
    /// run's script, as Claude Code ran it), how deep its spawn is (1 for one the session's own agent spawned), and
    /// whether it runs beside the turn that spawned it, which went on.
    pub agent_type: String,
    pub prompt: String,
    pub depth: i64,
    pub background: bool,
    /// A workflow run's: its name, the handle that resumes it (`Workflow({scriptPath, resumeFromRunId})`) once its launch
    /// is answered, where its script and its agents' transcripts are, and its agents as its progress last said.
    pub name: String,
    pub run_id: String,
    pub script_path: String,
    pub transcript_dir: String,
    pub agents: Vec<WorkflowAgent>,
}

/// One agent() call of a workflow run, as the run's progress says it. A workflow's agents run inside the run: they are not
/// subagents of the session, and what they write does not reach it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAgent {
    /// Its place among the run's agents, from 1.
    pub index: i64,
    /// The label the script gave it.
    pub label: String,
    /// The phase it ran in.
    pub phase: String,
    pub agent_id: String,
    /// Its subagent type.
    pub agent_type: String,
    /// running, done or error.
    pub state: String,
    /// The start of its prompt.
    pub prompt: String,
    /// The start of its result.
    pub result: String,
    pub error: String,
}

/// The end of what a background shell or a Monitor of a session's process has written: its last 8 KiB (`text`, as Claude
/// Code keeps it), all it wrote (`total_bytes`), and whether `text` starts partway through.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskOutput {
    pub text: String,
    pub total_bytes: u64,
    pub truncated: bool,
}

/// The agent asking to make a tool call, and why Claude Code asks rather than decides (empty when it gives no reason).
/// `classifier` is the auto-mode classifier's reason when the call is one it refused, come back to be asked (ADR-0083);
/// empty otherwise. `agent_id` is the subagent that asks; empty when the session's own agent does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequest {
    pub tool: ToolCall,
    pub reason: String,
    pub classifier: String,
    pub agent_id: String,
}

/// The answer to a [`PermissionRequest`]: the call is allowed, or refused with a message, which the agent reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(remote = "Self", rename_all = "snake_case")]
pub enum PermissionAnswer {
    Allow,
    Deny { message: String },
}

model::json_tagged!(PermissionAnswer, "answer");

/// An MCP server the session reaches, with the headers (the run's token) it is called with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    pub name: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
}

/// A subagent type a session may spawn (`agent::AgentType`): what it is for, its prompt, and what it runs with. `tools`
/// none inherits the parent's; `mcp` are servers of its own, beside the parent's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentType {
    pub description: String,
    pub prompt: String,
    pub tools: Option<Vec<String>>,
    pub deny_tools: Vec<String>,
    pub model: String,
    pub effort: String,
    pub skills: Vec<String>,
    pub mcp: Vec<McpServer>,
}

/// What a session is opened with. `sessionId` is minted by the core per open; `resume` is Claude's id of the conversation
/// to continue (empty: a new one); `tools` none leaves Claude's default tools, empty leaves none; `accountEnv` is the
/// environment a plugin gave for the session's account, empty for a login. `agents` are the subagent types it may spawn
/// beside Claude Code's built-ins; `builtinAgents` the built-ins it keeps, none keeping all of them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSpec {
    pub session_id: String,
    pub card_id: String,
    pub cwd: String,
    pub model: String,
    /// Claude Code's `--effort`; empty keeps its default.
    pub effort: String,
    pub account: String,
    pub account_env: BTreeMap<String, String>,
    pub mode: String,
    pub mcp: Vec<McpServer>,
    pub allowed_tools: Vec<String>,
    pub deny_tools: Vec<String>,
    pub ask_tools: Vec<String>,
    pub tools: Option<Vec<String>>,
    pub skills: Vec<String>,
    pub plugins: Vec<String>,
    pub system_append: String,
    pub resume: String,
    pub agents: BTreeMap<String, AgentType>,
    pub builtin_agents: Option<Vec<String>>,
    pub telemetry: Option<TelemetryExport>,
}

/// Where a session exports its OpenTelemetry, and the bearer token it takes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryExport {
    pub endpoint: String,
    pub token: String,
}

/// How a `session_open` ended. `refused`: why the session did not open, empty when it did; `failure`: what Claude said about
/// the account when the open failed on it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionOpened {
    pub resume_id: String,
    pub context_lost: bool,
    pub refused: String,
    pub failure: Option<Failure>,
}

/// Whether a steer reached the running turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Steered {
    pub steered: bool,
}

/// How a turn ended: `failure` is what Claude said about the account, `error` any other reason; both empty for a turn that
/// ended well.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnEnd {
    pub prompt_id: String,
    /// How a turn that was not an error ended; none when it was one.
    pub stop: Option<StopReason>,
    pub failure: Option<Failure>,
    pub error: String,
}

/// What an entry is, each kind with exactly its own field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum EntryBody {
    /// How the session's open ended.
    Opened {
        opened: SessionOpened,
    },
    Init {
        init: SessionInit,
    },
    Context {
        context: SessionContext,
    },
    Message {
        text: String,
    },
    Thought {
        text: String,
    },
    Tool {
        tool: ToolCall,
    },
    /// None: the plan stays as it was.
    Plan {
        plan: Option<Vec<model::PlanEntry>>,
    },
    Usage {
        usage: Usage,
    },
    Note {
        note: Note,
    },
    Task {
        task: Task,
    },
    Working {
        working: bool,
    },
    TurnEnded {
        turn: TurnEnd,
    },
    /// What the provider said of the account's limit; a warning comes before the limit rejects anything.
    Limit {
        limit: Limit,
    },
    /// Takes back a request the session no longer waits on.
    PermissionWithdrawn {
        request_id: String,
    },
    /// The last entry of a session that opened, and why it ended.
    Exit {
        exit: String,
    },
}

/// One thing a session did, numbered `seq` from 1 in the order it happened, per session. `agentId` is the subagent that
/// did it, empty for the session's own agent (and read as empty when absent): a subagent is spawned by the agent whose
/// `task` entry started it. A string is cut at 256 KiB by the runtime ("[cut: N bytes]").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEntry {
    pub seq: i64,
    #[serde(default)]
    pub agent_id: String,
    pub at: String,
    pub body: EntryBody,
}

/// A session the runtime still holds when it says hello, and the last entry it numbered for it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeldSession {
    pub session_id: String,
    pub last_seq: i64,
    pub working: bool,
}

/// What a runtime says when it connects: who it is, what build, which contract number, and the sessions it holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hello {
    pub runtime_id: String,
    pub revision: String,
    pub contract: i64,
    pub sessions: Vec<HeldSession>,
}

/// What the core has of a held session: every entry up to `acked` arrived.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionAck {
    pub session_id: String,
    pub acked: i64,
}

/// The core's answer to a hello. `refused` is why it will not take the runtime, empty when it will. A held session not
/// listed is killed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloReply {
    pub refused: String,
    pub sessions: Vec<SessionAck>,
}

/// What the agent of a session must do before its turn ends, which keeps it going in the same turn: empty when its turn
/// may end.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stopping {
    pub first: String,
}

/// A moment of an agent's work at which Claude Code takes what the core adds to what the agent reads (its hooks'
/// `additionalContext`): a call of a tool that succeeded, by its name and input, or the agent's start, a subagent's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Moment {
    ToolUsed {
        tool: String,
        #[serde(with = "model::stored_json::foreign_object")]
        input: Map<String, Value>,
    },
    Started,
}

/// What the agent reads beside a moment of its work: empty when nothing.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Told {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Acked {
    pub acked: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignIn {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Email {
    pub email: String,
    pub refused: String,
}

/// A part of a transcript file: `data` is base64 of at most 512 KiB from `offset`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptChunk {
    pub path: String,
    pub data: String,
    pub next: i64,
    pub eof: bool,
    pub missing: bool,
}

/// What the runtime calls on the core.
pub mod to_core {
    use super::{Ack, Acked, Hello, HelloReply, Moment, PermissionRequest, SessionEntry, Stopping, Told};

    link::contract! {
        capability: "runtime";
        reads {
            /// The session's own agent is about to end its turn (Claude Code's Stop hook): what it must do first.
            session_stopping(session_id: String) -> Stopping, why = "reads what the core asks of an agent before its turn ends";
        }
        changes {}
        idempotent {
            /// The first call of every session: the core answers with what it has of each held session, or refuses.
            runtime_hello(hello: Hello) -> HelloReply, why = "answers from the sessions the runtime names; a replay names the same and kills nothing more";
            /// Entries of a session, `seq` ascending; an entry the core has already is dropped.
            session_events(session_id: String, entries: Vec<SessionEntry>) -> Acked, why = "an entry the core has already is dropped by its seq";
            /// Answered by `permission_answered`, a call of its own. `after_seq` is the last entry the session wrote
            /// before it asked: the core asks once it has handed over every entry up to it, so a subagent's request
            /// follows the entry that spawned the subagent.
            permission_asked(session_id: String, request_id: String, request: PermissionRequest, after_seq: i64) -> Ack, why = "the same requestId asked again is one request";
            /// What the agent `agent_id` (empty: the session's own) reads beside moment of its work; asked, as
            /// `permission_asked` is, once the core has every entry up to `after_seq`, so a subagent's start follows the
            /// entry that spawned it. Answered within the call: Claude Code waits for it.
            session_moment(session_id: String, agent_id: String, after_seq: i64, moment: Moment) -> Told, why = "what a moment gives an agent joins what it was given, so the same moment again gives nothing twice";
            /// `error` is empty when the account is signed in, the reason when it is not.
            account_login_ended(login_id: String, error: String) -> Ack, why = "the same loginId ends once";
        }
        pushes {}
    }
}

/// What the core calls on the runtime.
pub mod to_runtime {
    use super::{
        Ack, Email, PermissionAnswer, Prompt, SessionContext, SessionSpec, SignIn, Steered, TaskOutput, TranscriptChunk,
    };
    use crate::app::SessionProcess;

    link::contract! {
        capability: "runtime.sessions";
        reads {
            /// A stopped card has no live session, so the transcript is keyed by `cwd` and `resume_id`.
            session_transcript(cwd: String, resume_id: String, offset: i64) -> TranscriptChunk, why = "reads a part of a transcript file";
            account_email(account: String) -> Email, why = "reads the email an account is signed in as";
            /// Reads the end of what one background shell or Monitor of the session's process has written, with no turn
            /// of the agent's.
            session_task_output(session_id: String, task_id: String) -> TaskOutput, why = "reads what a background task wrote";
            session_processes(session_id: String) -> Vec<SessionProcess>, why = "reads the OS process tree of a live session";
            session_context(session_id: String) -> SessionContext, why = "counts what fills a session's context";
        }
        changes {}
        idempotent {
            /// Starts opening a session and returns at once: how it ended comes as the session's `opened` entry.
            session_open(spec: SessionSpec) -> Ack, why = "a sessionId the runtime knows already is not opened again";
            /// The turn's end comes as a `turnEnded` entry naming `prompt_id`.
            session_prompt(session_id: String, prompt_id: String, prompt: Prompt) -> Ack, why = "the same promptId starts no second turn";
            session_steer(session_id: String, steer_id: String, prompt: Prompt) -> Steered, why = "the same steerId is not given twice";
            /// The session stays open and the turn's `turnEnded` entry follows.
            session_cancel(session_id: String) -> Ack, why = "a turn stopped twice stops once";
            /// Moves the running session onto the login of `account` without restarting it (ADR-0074); refused for a
            /// session of a plugin's account, which is closed and opened on the other account instead.
            session_move(session_id: String, account: String) -> Ack, why = "a session moved again to the same account takes that login again";
            /// Ends one task of the session's process (a subagent, a workflow run, a background shell or a Monitor); the
            /// turn that started it goes on, and the task's end follows as its `task` entry.
            session_stop_task(session_id: String, task_id: String) -> Ack, why = "a task stopped twice stops once";
            session_process_kill(session_id: String, kill_token: String) -> Ack, why = "a target identifies one OS process instance; a reused PID cannot be signaled by replay";
            /// Its `exit` entry follows once the process group has exited.
            session_kill(session_id: String) -> Ack, why = "a session ended twice ends once";
            permission_answered(session_id: String, request_id: String, answer: PermissionAnswer) -> Ack, why = "the same requestId answered again changes nothing";
            account_prepare(account: String) -> Ack, why = "a config dir made ready twice is ready";
            /// The end comes as `account_login_ended` for `login_id`.
            account_sign_in(login_id: String, account: String) -> SignIn, why = "the same loginId is one sign-in";
            account_login_code(login_id: String, code: String) -> Ack, why = "the same code given again to the same sign-in is one code";
            account_login_cancel(login_id: String) -> Ack, why = "a sign-in abandoned twice is abandoned";
            account_sign_out(account: String) -> Ack, why = "an account signed out twice is signed out";
        }
        pushes {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A moment crosses the link as postcard, which reads no tag it did not write: the tool's input, foreign JSON, arrives
    // as it left.
    #[test]
    fn a_moment_crosses_the_link_as_it_left() {
        let Value::Object(input) = serde_json::json!({ "file_path": "/w/crates/chains/src/session.rs", "limit": 40 })
        else {
            return;
        };
        let read = Moment::ToolUsed {
            tool: "Read".to_owned(),
            input,
        };
        for m in [read, Moment::Started] {
            let got = link::Payload::encode(&m)
                .map_err(|e| e.to_string())
                .and_then(|p| p.decode::<Moment>().map_err(|e| e.to_string()));
            assert_eq!(got, Ok(m));
        }
    }
}
