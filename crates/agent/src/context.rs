//! What fills a session's context in Remit's terms, from Claude Code's own count (`claudecode::ContextUsage`).

use claudecode::ContextUsage;
use model::{ContextItem, ContextPart, SessionContext};

/// Session `session_id`'s context as `u` counts it.
pub(crate) fn session_context(session_id: &str, u: ContextUsage) -> SessionContext {
    let item = |name: String, source: String, tokens: u64| ContextItem {
        name,
        source,
        tokens,
        deferred: false,
    };
    SessionContext {
        session_id: session_id.to_owned(),
        model: u.model,
        total: u.total_tokens,
        size: u.max_tokens,
        compact_at: u.auto_compact_threshold,
        parts: u
            .categories
            .into_iter()
            .map(|c| ContextPart {
                id: c.id,
                name: c.name,
                tokens: c.tokens,
                kind: c.kind,
            })
            .collect(),
        memory_files: u
            .memory_files
            .into_iter()
            .map(|f| item(f.path, f.r#type, f.tokens))
            .collect(),
        skills: u
            .skills
            .skill_frontmatter
            .into_iter()
            .map(|s| item(s.name, s.source, s.tokens))
            .collect(),
        agents: u
            .agents
            .into_iter()
            .map(|a| item(a.agent_type, a.source, a.tokens))
            .collect(),
        mcp_tools: u
            .mcp_tools
            .into_iter()
            .map(|t| ContextItem {
                deferred: !t.is_loaded,
                ..item(t.name, t.server_name, t.tokens)
            })
            .collect(),
        tool_calls: u
            .message_breakdown
            .tool_calls_by_type
            .into_iter()
            .map(|t| item(t.name, String::new(), t.call_tokens.saturating_add(t.result_tokens)))
            .collect(),
    }
}
