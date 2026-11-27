//! Runs one Claude Code process and speaks its SDK protocol to it: newline-delimited JSON over stdio (`--input-format`
//! and `--output-format stream-json`), with control requests both ways. It is the wire the official Agent SDKs speak; it
//! knows nothing of Remit. The process signs in as Claude Code on this machine does (its config dir): no key or token
//! passes here.
//!
//! Its environment is the names it needs of its parent's (`childenv::of`), never the whole. It runs as the machine's user
//! does, with the user's files and network (ADR-0138), as the leader of its own process group, which ends whole
//! (ADR-0018).

mod conn;
mod context;
mod login;
mod options;
mod status;
mod token;

use std::ffi::OsString;

pub use conn::{
    Conn, ExitError, Handler, Kind, Message, Permission, PermissionRequest, Priority, Stderr, TaskOutput, UserImage,
    UserMessage,
};
pub use context::{Agent, Category, ContextUsage, McpTool, MemoryFile, MessageBreakdown, Skill, Skills, ToolCalls};
pub use login::{Login, LoginOptions, start_login};
pub use options::{AgentDefinition, MCPServer, Options};
pub use status::{Auth, Status};
pub use token::{AccessToken, Store};

/// Why a request to this crate failed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no claude command")]
    NoCommand,
    #[error("no config dir")]
    NoConfigDir,
    #[error("encode: {0}")]
    Encode(#[from] serde_json::Error),
    #[error("start claude: {0}")]
    Start(#[source] procgroup::Error),
    /// The process ended; `said` is what the last result that was an error said, when one came before the end.
    #[error("claude code process ended{}", ended_with(said))]
    Closed { said: Vec<String> },
    #[error("claude code process ended: {0}")]
    Write(#[source] std::io::Error),
    /// The process answered a control request in a shape this client does not read.
    #[error("read claude's answer: {0}")]
    Answer(#[source] serde_json::Error),
    /// The process answered a control request with an error.
    #[error("{0}")]
    Refused(String),
    /// The process cannot hold a hook that fails closed (`OnFailure::Block`), so it does not start.
    #[error("claude code cannot hold the hooks that guard it, so it does not start: {0}")]
    Unguarded(&'static str),
    #[error("initialize: {0}")]
    Initialize(#[source] Box<Error>),
    /// A `claude auth` command failed.
    #[error("claude auth {subcommand}: {why}")]
    Auth { subcommand: String, why: String },
    #[error("read claude auth status: {0}")]
    Status(#[source] serde_json::Error),
    #[error("claude auth login ended before it gave a link: {0}")]
    LoginEnded(String),
    #[error("claude auth login gave no link in time: {0}")]
    NoLink(String),
    #[error("the code is the one line the sign-in page shows")]
    BadCode,
    #[error("the sign-in has ended: start it again")]
    SignInEnded,
    /// An account's stored login could not give a fresh access token.
    #[error("{0}")]
    Login(String),
    #[error("give the code to the sign-in: {0}")]
    GiveCode(#[source] std::io::Error),
}

impl Error {
    fn closed() -> Error {
        Error::Closed { said: Vec::new() }
    }
}

/// What `Error::Closed` says after its first words: nothing, or `: ` and the errors.
fn ended_with(said: &[String]) -> String {
    if said.is_empty() {
        return String::new();
    }
    format!(": {}", said.join("; "))
}

/// Gives cmd the environment every Claude Code child gets: the names `childenv` lets it take of this process's, then
/// extra, `NAME=value`, whose value of a name wins.
fn set_env<'a>(cmd: &mut procgroup::Command, extra: impl IntoIterator<Item = &'a String>) {
    let parent: Vec<String> = std::env::vars_os()
        .filter_map(|(k, v)| {
            let (k, v) = (k.into_string().ok()?, v.into_string().ok()?);
            Some(format!("{k}={v}"))
        })
        .collect();
    let extra: Vec<&String> = extra.into_iter().collect();
    cmd.env_clear();
    for kv in childenv::of(&parent, &extra) {
        if let Some((k, v)) = kv.split_once('=') {
            cmd.env(OsString::from(k), OsString::from(v));
        }
    }
}
