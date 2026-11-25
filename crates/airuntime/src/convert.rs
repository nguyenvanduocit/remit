//! What the runtime does to what a session reports before it crosses (strings cut to `FIELD_LIMIT`), and the session
//! spec and the ends of an open and a turn in the contract's words. What a session reports is the contract's own type on
//! both sides (ADR-0107 decision 4): nothing here copies it.

use agent::Spec;
use model::StopReason;
use serde_json::Value;
use wire::runtime::{self, PermissionRequest, Task, ToolCall};

/// The most a string the runtime sends may hold; a longer string is cut and says so (`wire::runtime::SessionEntry`).
const FIELD_LIMIT: usize = 256 << 10;

/// `s`, cut to `FIELD_LIMIT` bytes when longer, ending with how long it was.
pub(crate) fn cut(s: String) -> String {
    if s.len() <= FIELD_LIMIT {
        return s;
    }
    let mark = format!("[cut: {} bytes]", s.len());
    let mut keep = FIELD_LIMIT.saturating_sub(mark.len());
    while !s.is_char_boundary(keep) {
        keep = keep.saturating_sub(1);
    }
    let mut out = s;
    out.truncate(keep);
    out.push_str(&mark);
    out
}

/// `v` with every string in it cut, objects and lists walked.
fn cut_all(v: Value) -> Value {
    match v {
        Value::String(s) => Value::String(cut(s)),
        Value::Array(items) => Value::Array(items.into_iter().map(cut_all).collect()),
        Value::Object(fields) => Value::Object(fields.into_iter().map(|(k, x)| (k, cut_all(x))).collect()),
        other => other,
    }
}

/// The session the core asked to open, as the launcher takes it. The variables of the session's account are parsed here
/// again, whatever the caller parsed (ADR-0071): none is an account that is a login, and one name outside
/// `agent::parse_account_env`'s list refuses the open, naming the variable and holding no value.
pub(crate) fn agent_spec(s: runtime::SessionSpec) -> Result<Spec, String> {
    let account_env = if s.account_env.is_empty() {
        agent::AccountEnv::default()
    } else {
        agent::parse_account_env(s.account_env)
            .map_err(|e| format!("the session of card {} on account {}: {e}", s.card_id, s.account))?
    };
    Ok(Spec {
        card_id: s.card_id,
        cwd: s.cwd,
        model: s.model,
        effort: s.effort,
        account: s.account,
        account_env,
        mode: s.mode,
        mcp: mcp_servers(s.mcp),
        allowed_tools: s.allowed_tools,
        deny_tools: s.deny_tools,
        ask_tools: s.ask_tools,
        tools: s.tools,
        skills: s.skills,
        read_only: s.read_only,
        plugins: s.plugins,
        system_append: s.system_append,
        resume: s.resume,
        agents: s
            .agents
            .into_iter()
            .map(|(name, t)| {
                let t = agent::AgentType {
                    description: t.description,
                    prompt: t.prompt,
                    tools: t.tools,
                    deny_tools: t.deny_tools,
                    model: t.model,
                    effort: t.effort,
                    skills: t.skills,
                    mcp: mcp_servers(t.mcp),
                };
                (name, t)
            })
            .collect(),
        builtin_agents: s.builtin_agents,
        telemetry: s.telemetry.map(|t| agent::Telemetry {
            endpoint: t.endpoint,
            token: t.token,
        }),
    })
}

fn mcp_servers(servers: Vec<runtime::McpServer>) -> Vec<agent::MCPServer> {
    servers
        .into_iter()
        .map(|m| agent::MCPServer {
            name: m.name,
            url: m.url,
            headers: m.headers,
        })
        .collect()
}

/// How the start ended, as the core reads it: the session, or why it did not open (the provider's failure, said in its
/// own shape, or any other reason).
pub(crate) fn opened(started: &Result<Box<dyn agent::Session>, agent::Error>) -> runtime::SessionOpened {
    let mut out = runtime::SessionOpened {
        resume_id: String::new(),
        context_lost: false,
        refused: String::new(),
        failure: None,
    };
    match started {
        Ok(sess) => {
            out.resume_id = sess.id().to_owned();
            out.context_lost = sess.context_lost();
        }
        Err(agent::Error::Failure(f)) => out.failure = Some(f.clone()),
        Err(e) => out.refused = e.to_string(),
    }
    out
}

/// How the turn the prompt `prompt_id` started ended.
pub(crate) fn turn_end(prompt_id: String, ended: Result<StopReason, agent::Error>) -> runtime::TurnEnd {
    let mut out = runtime::TurnEnd {
        prompt_id,
        stop: None,
        failure: None,
        error: String::new(),
    };
    match ended {
        Ok(stop) => out.stop = Some(stop),
        Err(agent::Error::Failure(f)) => out.failure = Some(f),
        Err(e) => out.error = cut(e.to_string()),
    }
    out
}

/// The tool call with its title, input and output cut.
pub(crate) fn tool_call(mut t: ToolCall) -> ToolCall {
    t.title = cut(t.title);
    t.input = t.input.into_iter().map(|(k, v)| (k, cut_all(v))).collect();
    t.output = cut(t.output);
    t
}

/// The request with its tool call, reason and classifier cut.
pub(crate) fn permission_request(mut r: PermissionRequest) -> PermissionRequest {
    r.tool = tool_call(r.tool);
    r.reason = cut(r.reason);
    r.classifier = cut(r.classifier);
    r
}

/// The plan with each entry's text cut.
pub(crate) fn plan(mut p: Vec<model::PlanEntry>) -> Vec<model::PlanEntry> {
    for e in &mut p {
        e.content = cut(std::mem::take(&mut e.content));
    }
    p
}

/// The task with its words and its workflow agents' cut.
pub(crate) fn task(mut t: Task) -> Task {
    t.title = cut(t.title);
    t.summary = cut(t.summary);
    t.prompt = cut(t.prompt);
    for a in &mut t.agents {
        a.prompt = cut(std::mem::take(&mut a.prompt));
        a.result = cut(std::mem::take(&mut a.result));
        a.error = cut(std::mem::take(&mut a.error));
    }
    t
}

/// The init with each plugin error's message cut.
pub(crate) fn session_init(mut i: model::SessionInit) -> model::SessionInit {
    for e in &mut i.plugin_errors {
        e.message = cut(std::mem::take(&mut e.message));
    }
    i
}
