//! The `hooks` block of a Claude Code `settings.json`: event → matcher groups → handlers.
//!
//! Source: `Hm()` in Claude Code 2.1.322 (`chunk-mttfbqwj.js`). Fields marked internal upstream (`cloud`,
//! `rewakeMessage`, `rewakeSummary`) are left out.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// `"hooks": { "<Event>": [ { "matcher": ..., "hooks": [ ... ] } ] }`.
pub type Hooks = BTreeMap<EventName, Vec<MatcherGroup>>;

/// The 33 events a hook can be registered for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EventName {
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    PostToolBatch,
    Notification,
    UserPromptSubmit,
    UserPromptExpansion,
    SessionStart,
    SessionEnd,
    Stop,
    StopFailure,
    SubagentStart,
    SubagentStop,
    PreCompact,
    PostCompact,
    PreModelSwitch,
    PostModelSwitch,
    PermissionRequest,
    PermissionDenied,
    Setup,
    TeammateIdle,
    TaskCreated,
    TaskCompleted,
    Elicitation,
    ElicitationResult,
    ConfigChange,
    WorktreeCreate,
    WorktreeRemove,
    InstructionsLoaded,
    CwdChanged,
    FileChanged,
    DirectoryAdded,
    MessageDisplay,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatcherGroup {
    /// Empty or `*`: everything. Only `[A-Za-z0-9_|]`: exact names split on `|` (tool events also take `,`, space,
    /// `-`). Otherwise a regex. Ignored by events without matcher values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    pub hooks: Vec<Handler>,
}

/// One hook to run, by `type`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum Handler {
    /// A shell command (or, with `args`, an executable spawned without a shell) fed the input on stdin.
    Command {
        command: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        args: Option<Vec<String>>,
        /// A permission rule such as `Bash(git *)`: the hook runs only for matching tool calls.
        #[serde(rename = "if", default, skip_serializing_if = "Option::is_none")]
        condition: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shell: Option<Shell>,
        /// Seconds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        on_failure: Option<OnFailure>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status_message: Option<String>,
        /// Run once, then remove.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        once: Option<bool>,
        /// Run in the background without blocking.
        #[serde(rename = "async", default, skip_serializing_if = "Option::is_none")]
        run_async: Option<bool>,
        /// Run in the background and wake the model on exit code 2. Implies `async`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        async_rewake: Option<bool>,
    },
    /// An LLM judges the input (`$ARGUMENTS`) and answers `{ok, reason?, impossible?}`.
    Prompt {
        prompt: String,
        #[serde(rename = "if", default, skip_serializing_if = "Option::is_none")]
        condition: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        /// The `continue` of the block produced when `ok` is false.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        continue_on_block: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status_message: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        once: Option<bool>,
    },
    /// A tool of an already configured MCP server; string values of `input` take `${path}` from the hook input.
    McpTool {
        server: String,
        tool: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input: Option<Map<String, Value>>,
        #[serde(rename = "if", default, skip_serializing_if = "Option::is_none")]
        condition: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status_message: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        once: Option<bool>,
    },
    /// The input is POSTed to `url`; header values expand only the variables in `allowed_env_vars`.
    Http {
        url: String,
        #[serde(rename = "if", default, skip_serializing_if = "Option::is_none")]
        condition: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        on_failure: Option<OnFailure>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        headers: Option<BTreeMap<String, String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        allowed_env_vars: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status_message: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        once: Option<bool>,
    },
    /// An agent verifies `prompt` (default model Haiku, timeout 60 s).
    Agent {
        prompt: String,
        #[serde(rename = "if", default, skip_serializing_if = "Option::is_none")]
        condition: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status_message: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        once: Option<bool>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shell {
    Bash,
    Powershell,
}

/// What a failed run (no start, timeout, exit other than 0 or 2, invalid JSON) does. `block` counts it as exit 2;
/// ignored for async hooks and on Stop, SubagentStop, TaskCompleted and TeammateIdle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnFailure {
    Continue,
    Block,
}
