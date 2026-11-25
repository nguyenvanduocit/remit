//! What fills a session's context, as Claude Code counts it for `/context` (`get_context_usage`): its own token counts
//! per part, the parts' items, and the newest request's usage. Read as 2.1.322 answers it; a field a version leaves out
//! reads as empty, and one it adds is passed over.

use serde::Deserialize;

/// Claude Code's answer to `get_context_usage`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ContextUsage {
    /// The parts, in Claude Code's order; their `used` ones add up to `total_tokens`.
    pub categories: Vec<Category>,
    pub total_tokens: u64,
    /// The context window.
    pub max_tokens: u64,
    /// Where the conversation is compacted.
    pub auto_compact_threshold: u64,
    pub model: String,
    pub memory_files: Vec<MemoryFile>,
    pub mcp_tools: Vec<McpTool>,
    pub agents: Vec<Agent>,
    pub skills: Skills,
    pub message_breakdown: MessageBreakdown,
}

/// One part of the context: `kind` says whether it is sent (`used`), listed by name until loaded (`deferred`), held
/// back for compaction (`buffer`) or left (`free`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub tokens: u64,
    pub kind: String,
}

/// An instruction file the session read; `type` is where it came from (Project, User, Local, ...).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct MemoryFile {
    pub path: String,
    pub r#type: String,
    pub tokens: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct McpTool {
    pub name: String,
    pub server_name: String,
    pub tokens: u64,
    /// Its schema is in the context; a deferred tool is listed by name until the agent loads it.
    pub is_loaded: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Agent {
    pub agent_type: String,
    pub source: String,
    pub tokens: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Skills {
    pub skill_frontmatter: Vec<Skill>,
}

/// A skill as the listing carries it: its name and description, not its body.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Skill {
    pub name: String,
    pub source: String,
    pub tokens: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MessageBreakdown {
    pub tool_calls_by_type: Vec<ToolCalls>,
}

/// The calls of one tool in the conversation and their results.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ToolCalls {
    pub name: String,
    pub call_tokens: u64,
    pub result_tokens: u64,
}
