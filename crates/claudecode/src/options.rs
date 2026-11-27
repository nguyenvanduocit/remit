use std::collections::BTreeMap;

use claudehooks::config::{EventName, OnFailure};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// An MCP server the session is given, in Claude Code's `--mcp-config` shape: a process over stdio, or an endpoint over
/// http or sse. The variant is the shape's `type`, so a server cannot carry both a command and a url.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum MCPServer {
    Stdio {
        command: String,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        args: Vec<String>,
        #[serde(skip_serializing_if = "BTreeMap::is_empty")]
        env: BTreeMap<String, String>,
    },
    Http {
        url: String,
        #[serde(skip_serializing_if = "BTreeMap::is_empty")]
        headers: BTreeMap<String, String>,
        /// How long Claude Code waits for the answer of one call, in milliseconds: 60 s at least, which is also its
        /// wait for a value under 1000.
        timeout: u32,
    },
    Sse {
        url: String,
        #[serde(skip_serializing_if = "BTreeMap::is_empty")]
        headers: BTreeMap<String, String>,
    },
}

/// What a process is started with. Its default is Claude Code's defaults.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// The Claude Code CLI and any arguments of its own, e.g. `["claude"]`.
    pub command: Vec<String>,
    pub cwd: String,
    /// The process's environment besides the names it takes from its parent's (`childenv::of`), `NAME=value`.
    pub env: Vec<String>,
    pub model: String,
    /// `--effort`: low, medium, high, xhigh or max, checked by the caller; empty leaves Claude Code's default.
    pub effort: String,
    pub permission_mode: String,
    /// The session to go on with; empty starts a new one.
    pub resume: String,
    /// The id a new session takes; empty lets Claude Code choose.
    pub session_id: String,
    pub add_dirs: Vec<String>,
    /// By name; a `BTreeMap` so the command line is the same from one start to the next.
    pub mcp_servers: BTreeMap<String, MCPServer>,
    /// Only `mcp_servers`: none from the user's or the project's configuration.
    pub strict_mcp: bool,
    /// Which settings files it reads (user, project, local); `None` reads its default, empty none.
    pub setting_sources: Option<Vec<String>>,
    /// The only built-in tools the session has; `None` keeps all of them, empty keeps none.
    pub tools: Option<Vec<String>>,
    /// Rules that never ask for permission.
    pub allowed_tools: Vec<String>,
    /// Rules that always refuse.
    pub disallowed_tools: Vec<String>,
    /// A settings object laid over the files it reads (`--settings`), e.g. permission ask rules.
    pub settings: Map<String, Value>,
    /// Local plugin directories.
    pub plugins: Vec<String>,
    /// How the model's thinking streams: "summarized" asks recent models, which omit it otherwise, for a summary. Empty
    /// leaves it to Claude Code.
    pub thinking_display: String,
    /// What the initialize request carries.
    pub append_system_prompt: String,
    /// `Some(false)` renders the system prompt afresh at every request rather than reusing the one a resumed
    /// conversation recorded, so an appended prompt edited since arrives. `None` leaves it to Claude Code.
    pub system_prompt_snapshot: Option<bool>,
    /// The skills the model may use. `None` leaves them all; empty gives none.
    pub skills: Option<Vec<String>>,
    /// The subagent types the session may spawn beside Claude Code's built-ins, by name. Which of them it may spawn is
    /// the tools' to say (`Agent(name,...)`).
    pub agents: BTreeMap<String, AgentDefinition>,
    /// Streams what a subagent writes, each message naming it (`agent_id`); without it a subagent's text stays inside the
    /// tool call that spawned it.
    pub forward_subagent_text: bool,
    /// The hook events the handler answers (`Handler::hook`), for every tool and matcher, each with what a failed
    /// answer does: Claude Code asks over the control channel (`hook_callback`), before or beside the hooks of the
    /// settings it reads. `Block` fails closed (a rail), `Continue` goes on as without the hook.
    pub hooks: BTreeMap<EventName, OnFailure>,
}

/// One event's hooks as initialize registers them: the callback ids Claude Code names when it asks, and what an answer
/// that fails (an error, or one Claude Code cannot read) does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct HookMatcher {
    #[serde(rename = "hookCallbackIds")]
    pub hook_callback_ids: [EventName; 1],
    #[serde(rename = "onFailure")]
    pub on_failure: OnFailure,
}

/// A subagent type, in the SDK's fields (AgentDefinition): what the type is for (`description`, which the spawning model
/// reads), its system prompt, and what it runs with. `tools` `None` inherits the parent's, empty gives none; `mcp_servers`
/// are servers of its own, each `{name: server}`, beside the parent's.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDefinition {
    pub description: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub disallowed_tools: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub model: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub effort: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mcp_servers: Vec<BTreeMap<String, MCPServer>>,
}

/// A control request this client sends, by its `subtype`.
#[derive(Debug, Serialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub(crate) enum Request {
    Initialize {
        #[serde(rename = "appendSystemPrompt", skip_serializing_if = "Option::is_none")]
        append_system_prompt: Option<String>,
        #[serde(rename = "systemPromptSnapshot", skip_serializing_if = "Option::is_none")]
        system_prompt_snapshot: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        skills: Option<Vec<String>>,
        #[serde(skip_serializing_if = "BTreeMap::is_empty")]
        agents: BTreeMap<String, AgentDefinition>,
        #[serde(rename = "forwardSubagentText", skip_serializing_if = "std::ops::Not::not")]
        forward_subagent_text: bool,
        /// An event's one callback is named after the event.
        #[serde(skip_serializing_if = "BTreeMap::is_empty")]
        hooks: BTreeMap<EventName, [HookMatcher; 1]>,
    },
    Interrupt,
    SetModel {
        model: String,
    },
    SetPermissionMode {
        mode: String,
    },
    StopTask {
        task_id: String,
    },
    GetTaskOutput {
        task_id: String,
    },
    GetContextUsage,
}

impl Options {
    /// The command line of a process started with these options, after `command`. Every session reads stream-json in
    /// and out, asks its permissions over the control channel (canUseTool), and streams partial messages.
    pub(crate) fn args(&self) -> Result<Vec<String>, serde_json::Error> {
        let mut a: Vec<String> = [
            "--output-format",
            "stream-json",
            "--verbose",
            "--input-format",
            "stream-json",
            "--permission-prompt-tool=stdio",
            "--include-partial-messages",
        ]
        .map(String::from)
        .into();
        if !self.thinking_display.is_empty() {
            a.extend([
                "--thinking=adaptive".into(),
                "--thinking-display".into(),
                self.thinking_display.clone(),
            ]);
        }
        add(&mut a, "--model", &self.model);
        add(&mut a, "--effort", &self.effort);
        add(&mut a, "--permission-mode", &self.permission_mode);
        add(&mut a, "--resume", &self.resume);
        add(&mut a, "--session-id", &self.session_id);
        if let Some(tools) = &self.tools {
            a.extend(["--tools".into(), tools.join(",")]);
        }
        add(&mut a, "--allowedTools", &self.allowed_tools.join(","));
        add(&mut a, "--disallowedTools", &self.disallowed_tools.join(","));
        if !self.mcp_servers.is_empty() {
            let config = serde_json::json!({ "mcpServers": self.mcp_servers });
            a.extend(["--mcp-config".into(), serde_json::to_string(&config)?]);
        }
        if self.strict_mcp {
            a.push("--strict-mcp-config".into());
        }
        if let Some(sources) = &self.setting_sources {
            a.extend(["--setting-sources".into(), sources.join(",")]);
        }
        if !self.settings.is_empty() {
            a.extend(["--settings".into(), serde_json::to_string(&self.settings)?]);
        }
        for dir in &self.add_dirs {
            a.extend(["--add-dir".into(), dir.clone()]);
        }
        for dir in &self.plugins {
            a.extend(["--plugin-dir".into(), dir.clone()]);
        }
        Ok(a)
    }

    /// The initialize request a process started with these options is sent first.
    pub(crate) fn initialize(&self) -> Request {
        Request::Initialize {
            append_system_prompt: Some(self.append_system_prompt.clone()).filter(|p| !p.is_empty()),
            system_prompt_snapshot: self.system_prompt_snapshot,
            skills: self.skills.clone(),
            agents: self.agents.clone(),
            forward_subagent_text: self.forward_subagent_text,
            hooks: self
                .hooks
                .iter()
                .map(|(&e, &on_failure)| {
                    (
                        e,
                        [HookMatcher {
                            hook_callback_ids: [e],
                            on_failure,
                        }],
                    )
                })
                .collect(),
        }
    }
}

/// What a process's initialize answer says of the hooks the request carried. Each is absent from a CLI that predates
/// it: one that predates `hooks_applied` may have ignored the hooks, one that predates `hook_on_failure_supported`
/// ignores `onFailure`, so a hook's failure lets the action it guards go ahead.
#[derive(Debug, Deserialize)]
pub(crate) struct HooksTaken {
    #[serde(default)]
    hooks_applied: Option<bool>,
    #[serde(default)]
    hook_on_failure_supported: Option<bool>,
}

impl Options {
    /// Says why a process whose initialize answer is taken cannot hold the hooks that fail closed (`OnFailure::Block`):
    /// such a hook is a rail, and a process that dropped it or would let its failure through runs without that rail.
    /// None when it holds them, or none was asked for.
    pub(crate) fn unheld_hooks(&self, taken: &HooksTaken) -> Option<&'static str> {
        if !self.hooks.values().any(|&f| f == OnFailure::Block) {
            return None;
        }
        if taken.hooks_applied != Some(true) {
            return Some("it did not say it registered them");
        }
        if taken.hook_on_failure_supported != Some(true) {
            return Some("it does not honour onFailure, so a failed hook would let the action through");
        }
        None
    }
}

/// Adds flag with value to a command line, unless value is empty.
fn add(a: &mut Vec<String>, flag: &str, value: &str) {
    if !value.is_empty() {
        a.extend([flag.to_string(), value.to_string()]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn taken(applied: Option<bool>, on_failure: Option<bool>) -> HooksTaken {
        HooksTaken {
            hooks_applied: applied,
            hook_on_failure_supported: on_failure,
        }
    }

    fn with(on_failure: OnFailure) -> Options {
        Options {
            hooks: BTreeMap::from([(EventName::PreToolUse, on_failure)]),
            ..Options::default()
        }
    }

    #[test]
    fn a_process_that_cannot_hold_a_closing_hook_is_refused() {
        let guarded = with(OnFailure::Block);
        assert!(guarded.unheld_hooks(&taken(Some(true), Some(true))).is_none());
        // an older CLI leaves the fields out; a joining client gets hooks_applied false
        for t in [
            taken(None, Some(true)),
            taken(Some(false), Some(true)),
            taken(Some(true), None),
        ] {
            assert!(guarded.unheld_hooks(&t).is_some(), "{t:?}");
        }
        // hooks that fail open, or none, ask nothing of the process
        assert!(with(OnFailure::Continue).unheld_hooks(&taken(None, None)).is_none());
        assert!(Options::default().unheld_hooks(&taken(None, None)).is_none());
    }
}
