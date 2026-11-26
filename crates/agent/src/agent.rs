use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::Deserialize;

use model::{Prompt, StopReason};
use wire::runtime::{Limit, Moment, Note, PermissionAnswer, PermissionRequest, Task, TaskOutput, ToolCall, Usage};

use crate::tasks::WorkflowEntry;
use crate::{AccountEnv, Error};

/// A future a trait object can return.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Receives what one session's agent does; every start gets its own, so a callback is about the session it was given to.
/// The orchestrator implements it.
pub trait Handler: Send + Sync + 'static {
    /// Something the session did that its stream shows, in the order it happened.
    fn on_update(&self, u: Update);
    /// Asks whether the agent may make the tool call it is about to make. The future is dropped when the agent no longer
    /// waits for the answer (the turn was interrupted, the session ended).
    fn on_permission(&self, req: PermissionRequest) -> BoxFuture<'_, PermissionAnswer>;
    /// What Claude Code says the session loaded, at the start of each of its turns.
    fn on_init(&self, init: model::SessionInit);
    /// What fills the session's context, as Claude Code counts it, each time the session goes to rest. A handler that does
    /// not override it hears none.
    fn on_context(&self, _context: model::SessionContext) {}
    /// A task the session's process runs in the background (Claude Code's tasks: a background shell, a Monitor), when it
    /// starts, when more is known of it and when it ends.
    fn on_task(&self, t: Task);
    /// The agent started working (true) or went idle (false): in a turn a prompt started, or in one it took on its own,
    /// on news from a background task. Each report is a change.
    fn on_working(&self, working: bool);
    /// The process of a started session ended, after everything it wrote was reported: `None` for a clean exit. A session
    /// whose start failed is reported by its error alone.
    fn on_exit(&self, end: Option<Exit>);
    /// What the provider said of the account's limit at a turn of the session, each time it says it: a warning comes
    /// before the limit rejects anything. A handler that does not override it hears none.
    fn on_limit(&self, _limit: Limit) {}
    /// The handler of the subagent `agent_id`, which this handler's agent spawned (its task, of kind `TASK_SUBAGENT`, came
    /// to `on_task` first): what the subagent writes, the tools it calls, the permissions it asks and the tasks it starts,
    /// its own spawns among them, go to the handler returned. It is asked once per subagent, and must not call back into
    /// the session. `None`, as a handler that does not override it gives: what the subagent writes is dropped, and its
    /// permission requests are asked here.
    fn on_subagent(&self, _agent_id: &str) -> Option<Arc<dyn Handler>> {
        None
    }
    /// The agent is about to end its turn: what it still has to do first, which it is told and goes on to do in the same
    /// turn, or `None` to let the turn end. Asked once per turn: the turn it kept going ends when the agent next stops. A
    /// handler that does not override it lets every turn end.
    fn on_stop(&self) -> BoxFuture<'_, Option<String>> {
        Box::pin(async { None })
    }
    /// What the agent reads beside moment m of its work (a call of a tool that succeeded, its start as a subagent): added
    /// to its context there, or `None`. The agent waits for it, so it answers within a moment's wait; a handler that does
    /// not override it adds nothing.
    fn on_moment(&self, _m: Moment) -> BoxFuture<'_, Option<String>> {
        Box::pin(async { None })
    }
}

/// Why a started session's process ended other than cleanly.
#[derive(Debug, Clone, thiserror::Error)]
pub enum Exit {
    /// The process on this machine ended, as claudecode saw it.
    #[error(transparent)]
    Process(#[from] claudecode::ExitError),
    /// The process ran in an AI runtime, which reported its end in these words.
    #[error("{0}")]
    Reported(String),
    /// The AI runtime that held the session ended under it: the session's work is cut, not failed, and resumes at its
    /// next start.
    #[error("the AI runtime that held the session restarted: the session is gone, and resumes at its next start")]
    HostRestarted,
}

/// One thing a session did that its stream shows.
#[derive(Debug, Clone, PartialEq)]
pub enum Update {
    /// The next piece of the agent's answer.
    MessageChunk(String),
    /// The next piece of the agent's thinking.
    ThoughtChunk(String),
    /// A tool call, each time it starts or changes, as it now is.
    ToolCall(ToolCall),
    /// The agent's whole plan as it now is.
    Plan(Vec<model::PlanEntry>),
    /// How full the session's context is.
    Usage(Usage),
    /// Something Claude Code did to the session that is not the agent's own work.
    Note(Note),
}

/// The endpoint a session reaches Remit's own tools at, over http: a URL and the headers that carry its token.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MCPServer {
    pub name: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
}

/// What a session is started with. Its default is the narrowest Claude Code session: nothing of the user's own Claude
/// Code configuration, no skill, every built-in tool.
#[derive(Debug, Clone, Default)]
pub struct Spec {
    pub card_id: String,
    pub cwd: String,
    /// Claude Code's model; empty keeps its default.
    pub model: String,
    /// Claude Code's effort (`provider::Effort`, checked by the caller); empty keeps its default.
    pub effort: String,
    /// The account the session runs on (`model::Account`'s id): its config dir, and its login when it is one.
    pub account: String,
    /// The variables the account's plugin gave for this session (ADR-0071); its default is an account that is a login.
    /// It holds a credential: it is carried to the process that starts the session and written nowhere.
    pub account_env: AccountEnv,
    /// Claude Code's permission mode; empty keeps its default.
    pub mode: String,
    pub mcp: Vec<MCPServer>,
    /// Tools that never ask for permission, e.g. "mcp__remit".
    pub allowed_tools: Vec<String>,
    /// Claude Code deny rules, enforced in every permission mode, e.g. "Bash(git push:*)".
    pub deny_tools: Vec<String>,
    /// Claude Code ask rules: a request for what they match reaches the handler in every permission mode.
    pub ask_tools: Vec<String>,
    /// The only built-in tools the session has. `None` keeps all of them; empty keeps none.
    pub tools: Option<Vec<String>>,
    /// The skills the model may use, by canonical name (plugin:skill). Empty gives none.
    pub skills: Vec<String>,
    /// The paths the session reads and cannot change, beside the launcher's own: a judge whose ceiling holds no write
    /// tool lists its worktree here (ADR-0032).
    pub read_only: Vec<String>,
    /// Directories Claude Code loads as plugins: where the skills above come from.
    pub plugins: Vec<String>,
    /// Added to Claude Code's own system prompt. A resumed session takes the text it is given now.
    pub system_append: String,
    /// Session id to go on with; empty starts a new one.
    pub resume: String,
    /// The subagent types the session may spawn beside Claude Code's built-ins, by name.
    pub agents: BTreeMap<String, AgentType>,
    /// The built-in subagent types (general-purpose, fork, Explore, ...) the session keeps; every other is denied. `None`
    /// keeps all of them.
    pub builtin_agents: Option<Vec<String>>,
    /// Where the session exports its OpenTelemetry (ADR-0085); none exports nothing.
    pub telemetry: Option<Telemetry>,
}

/// An OTLP/HTTP endpoint (`.../otlp`, under which `/v1/logs`, `/v1/metrics` and `/v1/traces` are taken) and the bearer
/// token it takes. The token is a credential: carried to the process that starts the session and written nowhere.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Telemetry {
    pub endpoint: String,
    pub token: String,
}

/// A subagent type a session may spawn: what it is for (`description`, which the spawning model reads to choose it), its
/// system prompt, and what it runs with. `tools` `None` inherits the parent's tools, empty gives none; `deny_tools` are
/// refused to it even so. `mcp` are servers of its own, beside the parent's; one named as a parent's server does not
/// replace it (the parent's wins), so a type's own server takes a name of its own and denies the parent's.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentType {
    pub description: String,
    pub prompt: String,
    pub tools: Option<Vec<String>>,
    pub deny_tools: Vec<String>,
    pub model: String,
    pub effort: String,
    pub skills: Vec<String>,
    pub mcp: Vec<MCPServer>,
}

/// A live agent session.
pub trait Session: Send + Sync {
    fn id(&self) -> &str;
    /// Sends prompt (its words and images) and waits for the turn that answers it, which ends with a stop reason or an
    /// error: `Error::Failure` when the provider failed it. Dropping the future stops waiting.
    fn prompt(&self, prompt: Prompt) -> BoxFuture<'_, Result<StopReason, Error>>;
    /// Puts prompt into the turn the agent is in, which reads it between its steps, and says so. False with no error: no
    /// turn is running, and the prompt has gone nowhere: send it as a prompt.
    fn steer(&self, prompt: Prompt) -> BoxFuture<'_, Result<bool, Error>>;
    fn cancel(&self) -> BoxFuture<'_, Result<(), Error>>;
    fn close(&self);
    /// The session to resume was missing and a new one replaced it: the caller must repeat its brief.
    fn context_lost(&self) -> bool;
    /// Moves the running session onto the login of account: its process, its conversation and the turn in progress go
    /// on, and its next request to the model is made as that account (ADR-0074). `Error::NotMovable` for a session whose
    /// account is a plugin's: close it and start it on the other account.
    fn move_to(&self, account: String) -> BoxFuture<'_, Result<(), Error>>;
    /// Ends one task the session's process runs (a subagent, a workflow run, a background shell); its notification
    /// follows, to `Handler::on_task`. A session that cannot reach its process's tasks refuses.
    fn stop_task(&self, task_id: String) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            Err(Error::TurnFailed(format!(
                "task {task_id} cannot be stopped by this session"
            )))
        })
    }
    /// Reads the current process tree from the host running the session.
    fn processes(&self) -> BoxFuture<'_, Result<Vec<wire::app::SessionProcess>, Error>> {
        Box::pin(async {
            Err(Error::TurnFailed(
                "this session cannot read its OS processes".to_owned(),
            ))
        })
    }
    /// Force kills a process instance the runtime listed for this session.
    fn kill_process(&self, _kill_token: String) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async {
            Err(Error::TurnFailed(
                "this session cannot force kill its OS processes".to_owned(),
            ))
        })
    }
    /// Counts what fills the session's context now, as Claude Code does for `/context`. A session that cannot ask its
    /// process refuses.
    fn context(&self) -> BoxFuture<'_, Result<model::SessionContext, Error>> {
        Box::pin(async { Err(Error::TurnFailed("this session cannot count its context".to_owned())) })
    }
    /// Reads the end of what one background shell or Monitor of the session's process has written, with no turn of the
    /// agent's; the process keeps it until it exits. A session that cannot reach its process's tasks refuses.
    fn task_output(&self, task_id: String) -> BoxFuture<'_, Result<TaskOutput, Error>> {
        Box::pin(async move {
            Err(Error::TurnFailed(format!(
                "task {task_id} cannot be read by this session"
            )))
        })
    }
}

/// Starts sessions. `ClaudeLauncher` starts Claude Code; tests use a fake.
pub trait Launcher: Send + Sync {
    fn start(&self, spec: Spec, h: Arc<dyn Handler>) -> BoxFuture<'_, Result<Box<dyn Session>, Error>>;
}

/// The part of a Claude Code system message Remit reads, in the SDK's own field names: an init (SDKSystemMessage,
/// subtype init), a state change (SDKSessionStateChangedMessage) or one of the task messages
/// (SDKBackgroundTasksChangedMessage, SDKTaskStartedMessage, SDKTaskProgressMessage, SDKTaskNotificationMessage).
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct SdkMessage {
    pub subtype: String,
    pub claude_code_version: String,
    pub model: String,
    #[serde(rename = "permissionMode")]
    pub permission_mode: String,
    pub output_style: String,
    pub tools: Option<Vec<String>>,
    pub mcp_servers: Option<Vec<model::MCPServerState>>,
    pub skills: Option<Vec<String>>,
    pub plugins: Option<Vec<model::Plugin>>,
    pub plugin_errors: Option<Vec<model::PluginError>>,
    pub agents: Option<Vec<String>>,
    pub slash_commands: Option<Vec<String>>,

    pub state: String,

    pub tasks: Vec<SdkTask>,
    pub task_id: String,
    pub task_type: String,
    pub tool_use_id: String,
    pub status: String,
    pub summary: String,
    pub output_file: String,
    /// Not the agent's work: Claude Code's own watchers.
    pub ambient: bool,
    // What task_started says of a subagent or a workflow run, and the subagent whose task it is (parent_task_id).
    pub description: String,
    pub subagent_type: String,
    pub prompt: String,
    pub is_backgrounded: bool,
    pub spawn_depth: i64,
    pub workflow_name: String,
    pub parent_task_id: String,
    /// The task_progress of a workflow run: its phases and agents.
    pub workflow_progress: Option<Vec<WorkflowEntry>>,
}

/// A task in the level of background tasks.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct SdkTask {
    pub task_id: String,
    pub task_type: String,
    pub description: String,
    pub ambient: bool,
}

impl SdkMessage {
    pub(crate) fn init(self, session_id: &str) -> model::SessionInit {
        model::SessionInit {
            session_id: session_id.to_string(),
            version: self.claude_code_version,
            model: self.model,
            permission_mode: self.permission_mode,
            output_style: self.output_style,
            tools: self.tools.unwrap_or_default(),
            mcp_servers: self.mcp_servers.unwrap_or_default(),
            skills: self.skills.unwrap_or_default(),
            plugins: self.plugins.unwrap_or_default(),
            plugin_errors: self.plugin_errors.unwrap_or_default(),
            agents: self.agents.unwrap_or_default(),
            slash_commands: self.slash_commands.unwrap_or_default(),
        }
    }
}
