use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use claudehooks::config::{EventName, OnFailure};
use serde_json::{Map, json};

use crate::agent::{AgentType, MCPServer, Spec};

/// The tools of Claude Code that change files; a deny rule that names one forbids every edit.
pub const EDIT_TOOLS: [&str; 4] = ["Edit", "Write", "MultiEdit", "NotebookEdit"];

/// The subagent types Claude Code brings (2.1.322's init.agents, and fork, which it does not list). One a later Claude
/// Code adds is not denied until it is named here; the core refuses a spawn of a type it did not offer.
const BUILTIN_AGENTS: [&str; 8] = [
    "claude",
    "claude-code-guide",
    "Explore",
    "fork",
    "general-purpose",
    "Plan",
    "statusline-setup",
    "web-fetch",
];

/// The smallest conversation, in tokens, compacted while its session is idle, before its prompt cache expires, so the
/// next prompt does not pay to read all of it again.
const COMPACT_AT_200K: u32 = 200_000;
/// Where a growing conversation is compacted: a model's own window (1M) lets it carry far more than the work needs, and
/// every request pays for all of it.
const COMPACT_AT_400K: u32 = 400_000;

/// How long Claude Code waits for the answer of one call to Remit's server, in milliseconds. Left to itself it gives up
/// after 60 s while the tool runs on, and the agent never reads the answer. The tools' own bounds stay under this, so a
/// tool that runs into its bound still answers its agent, with its own refusal: `open_repo` 180 s
/// (`shipping/src/repo.rs` `OPEN_TIMEOUT`), bringing an archived repository back 180 s (`chains/src/archive.rs`
/// `RESTORE_TIMEOUT`), `record_verdict` 120 s (`shipping/src/gate.rs`). A tool given a longer bound raises this with it.
const MCP_TIMEOUT_MS: u32 = 300_000;

/// What a Claude Code process for spec is started with, but its command, its session and its MCP servers. A session
/// reads the repository's settings and CLAUDE.md and nothing of the user's Claude Code configuration, and takes only the
/// MCP servers it is given, so the user's skills, agents, plugins and servers never reach it; the skills it is given are
/// listed, an empty list meaning none. Its thinking streams as a summary: recent models omit it otherwise. It compacts its
/// conversation at the sizes above. It may spawn the subagent types it is given, and the built-ins it keeps, and what a
/// subagent writes streams to it, naming the subagent. `root` is the repository the session works in (`repository_root`).
pub(crate) fn options(spec: &Spec, root: &Path) -> claudecode::Options {
    let mut settings = Map::new();
    // Claude Code reads the instruction files of every directory from its working directory up to /, so a session under
    // the server user's home reads theirs (~/.claude/CLAUDE.md) as the project's: only the repository's own reach it.
    settings.insert("claudeMdExcludes".into(), json!(instructions_above(root)));
    settings.insert("autoCompactWindow".into(), json!(COMPACT_AT_400K));
    // idleCompaction true keeps a repository's settings from switching idle compaction off; whether it runs at all is
    // Anthropic's to decide. Its minimum has no setting, only the variable below.
    settings.insert("idleCompaction".into(), json!(true));
    // Claude Code's auto memory is off in every session: an agent's memory is Remit's own, given with each turn (ADR-0117),
    // and auto memory would write Claude Code's per-project default, which another employee's session in the same worktree
    // would read.
    settings.insert("autoMemoryEnabled".into(), json!(false));
    if !spec.ask_tools.is_empty() {
        settings.insert("permissions".into(), json!({ "ask": spec.ask_tools }));
    }
    claudecode::Options {
        cwd: spec.cwd.clone(),
        model: spec.model.clone(),
        effort: spec.effort.clone(),
        permission_mode: spec.mode.clone(),
        strict_mcp: true,
        setting_sources: Some(vec!["project".into()]),
        tools: spec.tools.clone(),
        allowed_tools: spec.allowed_tools.clone(),
        disallowed_tools: spec.deny_tools.iter().cloned().chain(denied_agents(spec)).collect(),
        plugins: spec.plugins.clone(),
        thinking_display: "summarized".into(),
        append_system_prompt: spec.system_append.clone(),
        skills: Some(spec.skills.clone()),
        agents: agent_definitions(&spec.agents),
        forward_subagent_text: true,
        hooks: hooks(),
        settings,
        env: [format!("CLAUDE_CODE_IDLE_COMPACT_MIN_TOKENS={COMPACT_AT_200K}")]
            .into_iter()
            .chain(spec.telemetry.iter().flat_map(telemetry_env))
            .collect(),
        // Claude Code records a conversation's system prompt when it first renders it and reuses the record on resume,
        // so a brief edited since would never arrive: rendered afresh every request, it does.
        system_prompt_snapshot: (!spec.system_append.is_empty()).then_some(false),
        ..claudecode::Options::default()
    }
}

/// The variables that have Claude Code export its logs, metrics and spans to t (ADR-0085): over OTLP/HTTP JSON, tool
/// details included, the text of prompts and answers not (the stream holds it). Logs and spans go out every few seconds,
/// so a card's telemetry follows its work. Metrics are delta, the only temporality Datadog's intake takes (ADR-0119).
fn telemetry_env(t: &crate::Telemetry) -> Vec<String> {
    [
        ("CLAUDE_CODE_ENABLE_TELEMETRY", "1"),
        ("CLAUDE_CODE_ENHANCED_TELEMETRY_BETA", "1"),
        ("OTEL_LOGS_EXPORTER", "otlp"),
        ("OTEL_METRICS_EXPORTER", "otlp"),
        ("OTEL_TRACES_EXPORTER", "otlp"),
        ("OTEL_EXPORTER_OTLP_PROTOCOL", "http/json"),
        ("OTEL_EXPORTER_OTLP_ENDPOINT", t.endpoint.as_str()),
        ("OTEL_LOG_TOOL_DETAILS", "1"),
        ("OTEL_LOGS_EXPORT_INTERVAL", "5000"),
        ("OTEL_TRACES_EXPORT_INTERVAL", "5000"),
        ("OTEL_METRIC_EXPORT_INTERVAL", "30000"),
        ("OTEL_EXPORTER_OTLP_METRICS_TEMPORALITY_PREFERENCE", "delta"),
    ]
    .into_iter()
    .map(|(k, v)| format!("{k}={v}"))
    .chain([format!("OTEL_EXPORTER_OTLP_HEADERS=Authorization=Bearer {}", t.token)])
    .collect()
}

/// The repository a session started in `cwd` works in: the nearest directory up from it that holds `.git` (a clone's
/// directory or a worktree's file), `cwd` itself when none does (the Assistant's). Read with symlinks resolved, as
/// Claude Code reads its working directory.
pub(crate) fn repository_root(cwd: &str) -> PathBuf {
    let cwd = Path::new(cwd).canonicalize().unwrap_or_else(|_| PathBuf::from(cwd));
    cwd.ancestors()
        .find(|d| d.join(".git").exists())
        .map_or_else(|| cwd.clone(), Path::to_path_buf)
}

/// The instruction files Claude Code would read above `root`, as `claudeMdExcludes` takes them.
fn instructions_above(root: &Path) -> Vec<String> {
    root.ancestors()
        .skip(1)
        .flat_map(|d| {
            ["CLAUDE.md", "CLAUDE.local.md", ".claude/CLAUDE.md", ".claude/rules/**"]
                .map(|f| d.join(f).to_string_lossy().into_owned())
        })
        .collect()
}

/// The hooks a session's handler answers (claude.rs's `hook`), each with what a failed answer does. A rail fails closed:
/// a tool call or a settings change Remit could not judge does not happen.
fn hooks() -> BTreeMap<EventName, OnFailure> {
    BTreeMap::from([
        // a turn being cut short starts no tool call, even one the mode allows without asking
        (EventName::PreToolUse, OnFailure::Block),
        // a call the auto-mode classifier refuses comes back as a permission request (`refused`)
        (EventName::PermissionDenied, OnFailure::Continue),
        // the settings a session runs with change only between sessions
        (EventName::ConfigChange, OnFailure::Block),
        (EventName::InstructionsLoaded, OnFailure::Continue),
        (EventName::PostModelSwitch, OnFailure::Continue),
        // what the agent must do before its turn ends (the handler's `on_stop`); unanswered, the turn ends
        (EventName::Stop, OnFailure::Continue),
        // what the agent reads after a tool call and as a subagent starts (the handler's `on_moment`); unanswered, it
        // reads nothing more
        (EventName::PostToolUse, OnFailure::Continue),
        (EventName::SubagentStart, OnFailure::Continue),
    ])
}

/// The subagent types of a session as initialize takes them.
fn agent_definitions(types: &BTreeMap<String, AgentType>) -> BTreeMap<String, claudecode::AgentDefinition> {
    types
        .iter()
        .map(|(name, t)| {
            let mcp_servers = t
                .mcp
                .iter()
                .map(|m| BTreeMap::from([(m.name.clone(), remit_server(m.clone()))]))
                .collect();
            let definition = claudecode::AgentDefinition {
                description: t.description.clone(),
                prompt: t.prompt.clone(),
                tools: t.tools.clone(),
                disallowed_tools: t.deny_tools.clone(),
                model: t.model.clone(),
                effort: t.effort.clone(),
                skills: t.skills.clone(),
                mcp_servers,
            };
            (name.clone(), definition)
        })
        .collect()
}

/// A deny rule for each built-in subagent type the session does not keep: the subagent tool takes no allowlist
/// (`Agent(a,b)` in its tools removes the tool), and a deny rule refuses the spawn, in a Workflow script too.
fn denied_agents(spec: &Spec) -> Vec<String> {
    let Some(kept) = &spec.builtin_agents else {
        return Vec::new();
    };
    BUILTIN_AGENTS
        .into_iter()
        .filter(|name| !spec.agents.contains_key(*name) && !kept.iter().any(|k| k == name))
        .map(|name| format!("Agent({name})"))
        .collect()
}

/// servers in Claude Code's --mcp-config shape, by name.
pub(crate) fn mcp_config(servers: Vec<MCPServer>) -> BTreeMap<String, claudecode::MCPServer> {
    servers.into_iter().map(|m| (m.name.clone(), remit_server(m))).collect()
}

/// Remit's server as Claude Code takes one: over http, a call's answer waited for as long as `MCP_TIMEOUT_MS`.
fn remit_server(m: MCPServer) -> claudecode::MCPServer {
    claudecode::MCPServer::Http {
        url: m.url,
        headers: m.headers,
        timeout: MCP_TIMEOUT_MS,
    }
}

/// Begins the name of the variable that holds one header value of a server: REMIT_MCP_0, _1...
const MCP_VALUE_VAR: &str = "REMIT_MCP_";

/// servers with each header value replaced by `${NAME}`, NAME a variable of the agent's environment, and those variables.
/// The values are numbered in the order of the servers and of their names, so the command line does not change from one
/// start to the next. Values go by reference because the servers reach Claude Code as its --mcp-config argument, which
/// every process on the machine reads (ps), and Claude Code expands a `${NAME}` in a server's headers from its own
/// environment, which only processes of the same user read.
pub(crate) fn by_reference(servers: &[MCPServer]) -> (Vec<MCPServer>, Vec<String>) {
    let mut env = Vec::new();
    let out = servers
        .iter()
        .map(|m| {
            let headers = m
                .headers
                .iter()
                .map(|(name, value)| {
                    let var = format!("{MCP_VALUE_VAR}{}", env.len());
                    env.push(format!("{var}={value}"));
                    (name.clone(), format!("${{{var}}}"))
                })
                .collect();
            MCPServer {
                name: m.name.clone(),
                url: m.url.clone(),
                headers,
            }
        })
        .collect();
    (out, env)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_code_waits_for_remits_longest_tool() {
        let servers = mcp_config(vec![MCPServer {
            name: "remit".into(),
            url: "http://127.0.0.1:4320/mcp".into(),
            headers: BTreeMap::new(),
        }]);
        let config = serde_json::to_value(&servers).unwrap();
        assert_eq!(config["remit"]["type"], "http");
        // `open_repo` may take 180 s (shipping/src/repo.rs `OPEN_TIMEOUT`), the longest bound a tool sets itself
        let timeout = config["remit"]["timeout"].as_u64().unwrap();
        assert!(timeout >= 180_000, "Claude Code would give up after {timeout} ms");
    }
}
