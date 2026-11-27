//! The session port (`Launcher`, `Session`, `Handler`) and its Claude Code adapter: one Claude Code process per card
//! session, reporting what it does in Remit's terms: the text it writes, the tools it calls, the permissions it asks, its
//! plan, its usage, its background tasks. The core holds only the port and reaches the adapter through the AI runtime
//! (runtimelink, ADR-0058); `ClaudeLauncher` runs in the runtime alone. A session starts with no secret of the server in
//! its environment and with the paths that hold Remit's secrets hidden from it (`ClaudeLauncher`); the one credential it
//! holds is its account's: the provider login in its config dir, or the variables the account's plugin gave
//! (`AccountEnv`).

mod accountenv;
mod accounts;
mod agent;
mod claude;
mod context;
mod failure;
mod options;
mod process_targets;
mod processes;
mod refused;
mod subagents;
mod tasks;
mod toolcall;

pub use accountenv::{AccountEnv, AccountEnvError, parse_account_env};
pub use accounts::{ClaudeAccounts, Login};
pub use agent::{AgentType, BoxFuture, Exit, Handler, Launcher, MCPServer, Session, Spec, Telemetry, Update};
pub use claude::{ClaudeLauncher, Clock, Home, StoreOf};
pub use options::EDIT_TOOLS;
pub use subagents::Subagents;

/// Why a session, a turn or an account action failed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Claude(#[from] claudecode::Error),
    #[error("a turn is already running")]
    TurnRunning,
    /// The provider failed the turn: the work waits for the provider rather than for the owner.
    #[error(transparent)]
    Failure(#[from] wire::runtime::Failure),
    /// The turn ended in an error that is not the provider's.
    #[error("{0}")]
    TurnFailed(String),
    /// The process ended while a turn was open; `None` when it exited cleanly.
    #[error("the agent ended: {}", .0.as_ref().map_or_else(|| "claude code process ended".to_string(), ToString::to_string))]
    Ended(Option<Exit>),
    /// The AI runtime that held the session ended under it (it crashed, or was killed and started again): the session's
    /// work is cut, not failed, and its conversation resumes at the session's next start.
    #[error("the AI runtime that held the session restarted: the session is gone, and resumes at its next start")]
    HostRestarted,
    #[error(
        "the launcher names no config dir for the plugin account {0}: refusing to start its session beside the \
         machine's login"
    )]
    PluginAccountWithoutHome(String),
    #[error("{0}")]
    Prepare(String),
    #[error(
        "the session runs on a plugin's account, whose variables a running process cannot change: start it on the other account"
    )]
    NotMovable,
    #[error("it is not signed in with a Claude account")]
    NotSignedIn,
    #[error("the default account is the login of this machine: it is not discarded")]
    DiscardDefault,
}
