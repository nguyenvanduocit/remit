//! The session spec in the runtime contract's words, and how a session's end reads to the core. What a session reports
//! is the contract's own type on both sides (ADR-0107 decision 4): nothing here copies it.

use agent::{Exit, Spec};
use claudecode::ExitError;
use wire::runtime;

/// `spec` as the runtime is asked to open it under the core's `session_id`. `tools` none stays none (Claude's default
/// tools) and empty stays empty (none). The variables of a plugin's account go with it, values and all (ADR-0071): this
/// call is their one way to the process that starts the session.
pub(crate) fn session_spec(session_id: &str, spec: &Spec) -> runtime::SessionSpec {
    runtime::SessionSpec {
        session_id: session_id.to_owned(),
        card_id: spec.card_id.clone(),
        cwd: spec.cwd.clone(),
        model: spec.model.clone(),
        effort: spec.effort.clone(),
        account: spec.account.clone(),
        account_env: spec.account_env.vars().clone(),
        mode: spec.mode.clone(),
        mcp: mcp_servers(&spec.mcp),
        allowed_tools: spec.allowed_tools.clone(),
        deny_tools: spec.deny_tools.clone(),
        ask_tools: spec.ask_tools.clone(),
        tools: spec.tools.clone(),
        skills: spec.skills.clone(),
        read_only: spec.read_only.clone(),
        plugins: spec.plugins.clone(),
        system_append: spec.system_append.clone(),
        resume: spec.resume.clone(),
        agents: spec
            .agents
            .iter()
            .map(|(name, t)| {
                let t = runtime::AgentType {
                    description: t.description.clone(),
                    prompt: t.prompt.clone(),
                    tools: t.tools.clone(),
                    deny_tools: t.deny_tools.clone(),
                    model: t.model.clone(),
                    effort: t.effort.clone(),
                    skills: t.skills.clone(),
                    mcp: mcp_servers(&t.mcp),
                };
                (name.clone(), t)
            })
            .collect(),
        builtin_agents: spec.builtin_agents.clone(),
        telemetry: spec.telemetry.as_ref().map(|t| runtime::TelemetryExport {
            endpoint: t.endpoint.clone(),
            token: t.token.clone(),
        }),
    }
}

fn mcp_servers(servers: &[agent::MCPServer]) -> Vec<runtime::McpServer> {
    servers
        .iter()
        .map(|m| runtime::McpServer {
            name: m.name.clone(),
            url: m.url.clone(),
            headers: m.headers.clone(),
        })
        .collect()
}

/// Why a session the core holds ended.
#[derive(Debug, Clone)]
pub(crate) enum Ending {
    /// Its process exited: what the runtime said of it, none when it exited with nothing to say.
    Exited(Option<String>),
    /// The runtime that held it ended (it exited, or another runtime took its place): its conversation resumes at its
    /// next start.
    Restarted(String),
    /// The runtime came back without it.
    Unheld,
}

const UNHELD: &str = "the AI runtime no longer holds the session";

impl Ending {
    /// What the prompt still open when the session ended returns, and every prompt sent after.
    pub(crate) fn turn_error(&self) -> agent::Error {
        match self {
            Ending::Exited(why) => agent::Error::Ended(why.as_deref().map(exit_error)),
            Ending::Restarted(_) => agent::Error::HostRestarted,
            Ending::Unheld => agent::Error::Ended(Some(exit_error(UNHELD))),
        }
    }

    /// What the handler's `on_exit` hears.
    pub(crate) fn exit(&self) -> Option<Exit> {
        match self {
            Ending::Exited(why) => why.as_deref().map(exit_error),
            Ending::Restarted(_) => Some(Exit::HostRestarted),
            Ending::Unheld => Some(exit_error(UNHELD)),
        }
    }
}

/// How the runtime said a session's process exited: its words are all that crossed, so a stop reads back as `Stopped`
/// and anything else as the words it was.
fn exit_error(why: &str) -> Exit {
    if why == ExitError::Stopped.to_string() {
        Exit::Process(ExitError::Stopped)
    } else {
        Exit::Reported(why.to_owned())
    }
}
