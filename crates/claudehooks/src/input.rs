//! The JSON every hook receives: [`Common`] fields plus one [`Event`], tagged by `hook_event_name`.
//!
//! Source: the zod input schemas of Claude Code 2.1.322 (`chunk-smzekmsg.js`). A JSON number the schema does not
//! mark as an integer is an `f64`.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// One hook invocation's input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HookInput {
    #[serde(flatten)]
    pub common: Common,
    #[serde(flatten)]
    pub event: Event,
}

/// Fields every event carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Common {
    pub session_id: String,
    pub transcript_path: String,
    pub cwd: String,
    /// Joins a user prompt with every event until the next prompt; the OpenTelemetry `prompt.id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    /// Set only when the hook fires inside a subagent; always set for `SubagentStart` and `SubagentStop`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// The subagent's type, or the `--agent` of the main thread; always set for `SubagentStart` and `SubagentStop`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<Effort>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Effort {
    /// `low`, `medium`, `high`, `xhigh` or `max`, after any downgrade for the model.
    pub level: String,
}

/// The MCP server behind an `mcp__*` tool. `source` is an open set; trust it, not `name`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpServer {
    pub name: String,
    pub source: String,
}

/// The event and its own fields. `agent_id` and `agent_type` of the subagent events live in [`Common`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "hook_event_name")]
pub enum Event {
    PreToolUse {
        tool_name: String,
        tool_input: Value,
        tool_use_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mcp_server: Option<McpServer>,
    },
    PermissionRequest {
        tool_name: String,
        tool_input: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        permission_suggestions: Option<Vec<PermissionUpdate>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mcp_server: Option<McpServer>,
    },
    PostToolUse {
        tool_name: String,
        tool_input: Value,
        /// The tool's structured result (for Bash: `stdout`, `stderr`, `interrupted`, ...).
        tool_response: Value,
        tool_use_id: String,
        /// Execution time, without permission prompts and hooks.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ms: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mcp_server: Option<McpServer>,
    },
    PostToolUseFailure {
        tool_name: String,
        tool_input: Value,
        tool_use_id: String,
        error: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        is_interrupt: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ms: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mcp_server: Option<McpServer>,
    },
    /// Once per batch of tool calls, after all resolved and before the next model request.
    PostToolBatch {
        tool_calls: Vec<BatchedToolCall>,
    },
    /// Auto mode denied a tool call.
    PermissionDenied {
        tool_name: String,
        tool_input: Value,
        tool_use_id: String,
        reason: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mcp_server: Option<McpServer>,
    },
    Notification {
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// An open set (`permission_prompt`, `idle_prompt`, ... and plugin types).
        notification_type: String,
    },
    UserPromptSubmit {
        prompt: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<PromptSource>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_title: Option<String>,
    },
    UserPromptExpansion {
        expansion_type: ExpansionType,
        command_name: String,
        command_args: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        command_source: Option<String>,
        prompt: String,
    },
    SessionStart {
        source: SessionStartSource,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_title: Option<String>,
        /// Resume or fork only, as are the next three fields.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seconds_since_last_response: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        context_tokens: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prompt_cache_likely_expired: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        estimated_cache_write_usd: Option<f64>,
    },
    SessionEnd {
        reason: SessionEndReason,
    },
    Setup {
        trigger: SetupTrigger,
    },
    Stop {
        /// True when this stop follows a Stop hook that already blocked once: guard against loops with it.
        stop_hook_active: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        last_assistant_message: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        background_tasks: Option<Vec<BackgroundTask>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_crons: Option<Vec<SessionCron>>,
    },
    /// Fires in place of `Stop` when an API error ended the turn; its output is ignored.
    StopFailure {
        error: ApiError,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error_details: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        last_assistant_message: Option<String>,
    },
    SubagentStart {},
    SubagentStop {
        stop_hook_active: bool,
        agent_transcript_path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        last_assistant_message: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        background_tasks: Option<Vec<BackgroundTask>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_crons: Option<Vec<SessionCron>>,
    },
    PreCompact {
        trigger: CompactTrigger,
        custom_instructions: Option<String>,
    },
    PostCompact {
        trigger: CompactTrigger,
        compact_summary: String,
    },
    PreModelSwitch(ModelSwitch<RequestedSwitchSource>),
    PostModelSwitch(ModelSwitch<ModelSwitchSource>),
    TeammateIdle {
        teammate_name: String,
        /// Deprecated upstream: a session has one implicit team.
        team_name: String,
    },
    TaskCreated(Task),
    TaskCompleted(Task),
    /// An MCP server asks the user for input.
    Elicitation {
        mcp_server_name: String,
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode: Option<ElicitationMode>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        elicitation_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        requested_schema: Option<Map<String, Value>>,
    },
    ElicitationResult {
        mcp_server_name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        elicitation_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode: Option<ElicitationMode>,
        action: ElicitationAction,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content: Option<Map<String, Value>>,
    },
    ConfigChange {
        source: ConfigSource,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        file_path: Option<String>,
    },
    /// A CLAUDE.md or rule file was loaded. Observe-only: it cannot block.
    InstructionsLoaded {
        file_path: String,
        memory_type: MemoryType,
        load_reason: LoadReason,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        globs: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        trigger_file_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_file_path: Option<String>,
    },
    /// The hook creates the worktree and answers with its absolute path.
    WorktreeCreate {
        name: String,
    },
    WorktreeRemove {
        worktree_path: String,
    },
    CwdChanged {
        old_cwd: String,
        new_cwd: String,
    },
    FileChanged {
        file_path: String,
        event: FileEvent,
    },
    DirectoryAdded {
        directory: String,
        source: DirectorySource,
    },
    /// A batch of newly completed lines of a streaming assistant message. Display-only.
    MessageDisplay {
        turn_id: String,
        message_id: String,
        index: u64,
        #[serde(rename = "final")]
        is_final: bool,
        delta: String,
    },
    /// An event newer than this crate knows: answered with no opinion, so Claude Code goes on as without the hook.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchedToolCall {
    pub tool_name: String,
    pub tool_input: Value,
    pub tool_use_id: String,
    /// The result as the model sees it; a Bash call gives a string here, unlike `PostToolUse`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_response: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackgroundTask {
    pub id: String,
    /// `shell`, `subagent`, `monitor`, `workflow`, ...
    #[serde(rename = "type")]
    pub kind: String,
    pub status: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionCron {
    pub id: String,
    pub schedule: String,
    pub recurring: bool,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub task_id: String,
    pub task_subject: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teammate_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_name: Option<String>,
}

/// Input of `PreModelSwitch` (source `S` = [`RequestedSwitchSource`]) and `PostModelSwitch` ([`ModelSwitchSource`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelSwitch<S> {
    pub from_model: String,
    pub to_model: String,
    /// An alias, a full id, or `None` for "default".
    pub requested_model: Option<String>,
    pub source: S,
    pub context_tokens: f64,
    pub prompt_cache_warm: bool,
    pub cache_ttl: CacheTtl,
    pub estimated_cache_write_usd: f64,
    pub pricing: Pricing,
}

/// A change to permission rules, offered in `PermissionRequest` and returned by its `allow` decision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PermissionUpdate {
    AddRules {
        rules: Vec<PermissionRule>,
        behavior: RuleBehavior,
        destination: PermissionDestination,
    },
    ReplaceRules {
        rules: Vec<PermissionRule>,
        behavior: RuleBehavior,
        destination: PermissionDestination,
    },
    RemoveRules {
        rules: Vec<PermissionRule>,
        behavior: RuleBehavior,
        destination: PermissionDestination,
    },
    SetMode {
        mode: String,
        destination: PermissionDestination,
    },
    AddDirectories {
        directories: Vec<String>,
        destination: PermissionDestination,
    },
    RemoveDirectories {
        directories: Vec<String>,
        destination: PermissionDestination,
    },
    /// An update newer than this crate knows.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRule {
    pub tool_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_content: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleBehavior {
    Allow,
    Deny,
    Ask,
    /// A value newer than this crate knows.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionDestination {
    UserSettings,
    ProjectSettings,
    LocalSettings,
    Session,
    CliArg,
    /// A value newer than this crate knows.
    #[serde(other)]
    Other,
}

/// Who authored the prompt. `poll_event` blocks an incoming event before it is acknowledged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptSource {
    User,
    Sdk,
    System,
    LoopWakeup,
    ScheduleWakeup,
    PollEvent,
    /// A value newer than this crate knows.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpansionType {
    SlashCommand,
    McpPrompt,
    /// A value newer than this crate knows.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStartSource {
    Startup,
    Resume,
    Clear,
    Compact,
    Fork,
    /// A value newer than this crate knows.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEndReason {
    Clear,
    Resume,
    Logout,
    PromptInputExit,
    /// `other`, or a reason newer than this crate knows.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetupTrigger {
    Init,
    Maintenance,
    /// A value newer than this crate knows.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactTrigger {
    Manual,
    Auto,
    /// A value newer than this crate knows.
    #[serde(other)]
    Other,
}

/// Why the API ended the turn (`StopFailure`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiError {
    AuthenticationFailed,
    OauthOrgNotAllowed,
    AccountOnHold,
    VerificationRequired,
    BillingError,
    RateLimit,
    Overloaded,
    InvalidRequest,
    ModelNotFound,
    ServerError,
    MaxOutputTokens,
    CloudCredentialError,
    /// `unknown`, or an error newer than this crate knows.
    #[serde(other)]
    Unknown,
}

/// What asked for a model switch before it happens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestedSwitchSource {
    Command,
    Picker,
    Sdk,
    /// A value newer than this crate knows.
    #[serde(other)]
    Other,
}

/// What changed the model, including automatic fallback and resume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelSwitchSource {
    Command,
    Picker,
    Sdk,
    Auto,
    Resume,
    /// A value newer than this crate knows.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheTtl {
    #[serde(rename = "5m")]
    FiveMinutes,
    #[serde(rename = "1h")]
    OneHour,
    /// A value newer than this crate knows.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pricing {
    Configured,
    Catalog,
    Default,
    /// A value newer than this crate knows.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElicitationMode {
    Form,
    Url,
    /// A value newer than this crate knows.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElicitationAction {
    Accept,
    Decline,
    Cancel,
    /// A value newer than this crate knows.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigSource {
    UserSettings,
    ProjectSettings,
    LocalSettings,
    PolicySettings,
    Skills,
    /// A value newer than this crate knows.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryType {
    User,
    Project,
    Local,
    Managed,
    /// A value newer than this crate knows.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadReason {
    SessionStart,
    NestedTraversal,
    PathGlobMatch,
    Include,
    Compact,
    /// A value newer than this crate knows.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileEvent {
    Change,
    Add,
    Unlink,
    /// A value newer than this crate knows.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectorySource {
    SlashCommand,
    RegisterRepoRoot,
    /// A value newer than this crate knows.
    #[serde(other)]
    Other,
}
