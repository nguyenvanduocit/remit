//! The JSON a hook prints on stdout (exit 0) to steer Claude Code.
//!
//! Source: the zod output schemas of Claude Code 2.1.322 (`chunk-smzekmsg.js`) and its parser `r5`
//! (`chunk-9r1hbf9p.js`). How Claude Code reads them:
//! - `decision: block` acts as exit code 2 on every event, with `reason` as the message.
//! - `decision: approve` is "allow" where the event decides a permission.
//! - On `PreToolUse`, [`SpecificOutput::PreToolUse`]'s `permission_decision` is read after `decision` and wins.
//! - A `hookEventName` other than the event the hook ran for is an error.

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::input::{ElicitationAction, PermissionUpdate};

/// The answer of a hook that finishes before Claude Code goes on.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookOutput {
    /// `false` stops Claude after the hooks run; `stop_reason` is shown to the user.
    #[serde(rename = "continue", default, skip_serializing_if = "Option::is_none")]
    pub keep_going: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    /// Hides stdout from the transcript.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppress_output: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<Decision>,
    /// The message of a `block`, shown where the event sends a blocking error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// A warning shown to the user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_message: Option<String>,
    /// An OSC 0, 1, 2, 9, 99 or 777 sequence or BEL that Claude Code emits for the hook; anything else is dropped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_sequence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hook_specific_output: Option<SpecificOutput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    Approve,
    Block,
}

/// The answer of a hook that detaches: Claude Code goes on without waiting for it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsyncOutput {
    #[serde(rename = "async")]
    marker: AsyncMarker,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub async_timeout: Option<f64>,
}

impl AsyncOutput {
    pub fn new(async_timeout: Option<f64>) -> Self {
        Self {
            marker: AsyncMarker,
            async_timeout,
        }
    }
}

/// The literal `true` of `{"async": true}`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct AsyncMarker;

impl Serialize for AsyncMarker {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bool(true)
    }
}

impl<'de> Deserialize<'de> for AsyncMarker {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if bool::deserialize(deserializer)? {
            Ok(Self)
        } else {
            Err(de::Error::custom("`async` must be true"))
        }
    }
}

/// Fields only one event reads, tagged by `hookEventName`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "hookEventName", rename_all_fields = "camelCase")]
pub enum SpecificOutput {
    PreToolUse {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        permission_decision: Option<PermissionDecision>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        permission_decision_reason: Option<String>,
        /// Replaces the tool's input.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        updated_input: Option<Map<String, Value>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
    },
    PermissionRequest {
        decision: PermissionRequestDecision,
    },
    PostToolUse {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
        /// A short user statement for the auto-mode classifier; 2000 UTF-16 units shared by all hooks of one call.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        classifier_context: Option<String>,
        /// Replaces the tool output the model sees.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        updated_tool_output: Option<Value>,
        /// The MCP-only form of `updated_tool_output`.
        #[serde(rename = "updatedMCPToolOutput", default, skip_serializing_if = "Option::is_none")]
        updated_mcp_tool_output: Option<Value>,
    },
    PostToolUseFailure {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
    },
    PostToolBatch {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
    },
    /// `true` tells the model it may retry the denied call.
    PermissionDenied {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        retry: Option<bool>,
    },
    Notification {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
    },
    UserPromptSubmit {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_title: Option<String>,
        /// With `decision: block`, leaves the prompt out of the block message.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        suppress_original_prompt: Option<bool>,
    },
    UserPromptExpansion {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        suppress_original_prompt: Option<bool>,
    },
    SessionStart {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        initial_user_message: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_title: Option<String>,
        /// Absolute paths for the `FileChanged` watcher.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        watch_paths: Option<Vec<String>>,
        /// Re-scan skills after the hooks, so skills a hook installed load in this session.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reload_skills: Option<bool>,
    },
    Setup {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
    },
    Stop {
        /// Feedback to the model; the conversation continues so it can act on it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
    },
    SubagentStart {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
    },
    SubagentStop {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
    },
    PreModelSwitch {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        permission_decision: Option<SwitchDecision>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        permission_decision_reason: Option<String>,
    },
    PostModelSwitch {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        additional_context: Option<String>,
    },
    Elicitation {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        action: Option<ElicitationAction>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content: Option<Map<String, Value>>,
    },
    ElicitationResult {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        action: Option<ElicitationAction>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content: Option<Map<String, Value>>,
    },
    /// A command hook may print the bare path on stdout instead.
    WorktreeCreate {
        worktree_path: String,
    },
    CwdChanged {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        watch_paths: Option<Vec<String>>,
    },
    FileChanged {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        watch_paths: Option<Vec<String>>,
    },
    /// Shown in place of the delta; the stored message is unchanged.
    MessageDisplay {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        display_content: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionDecision {
    Allow,
    Deny,
    Ask,
    Defer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SwitchDecision {
    Allow,
    Deny,
    Ask,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "behavior", rename_all = "lowercase", rename_all_fields = "camelCase")]
pub enum PermissionRequestDecision {
    Allow {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        updated_input: Option<Map<String, Value>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        updated_permissions: Option<Vec<PermissionUpdate>>,
    },
    Deny {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
        /// Also interrupts the turn.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        interrupt: Option<bool>,
    },
}
