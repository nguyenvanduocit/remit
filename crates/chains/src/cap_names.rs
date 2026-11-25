//! The names of the capabilities in the catalog (capabilities.rs in the capability crate). The core reads some of them
//! (admission, the tool policy), so every name is defined here, below each crate that uses it.

pub const CAP_HISTORY: &str = "history";
pub const CAP_ISSUES: &str = "issues";
pub const CAP_PROPOSE: &str = "propose";
pub const CAP_PLAYBOOKS: &str = "playbooks";
pub const CAP_AGENTS: &str = "agents";
pub const CAP_LIBRARY: &str = "library";
pub const CAP_OUTCOMES: &str = "outcomes";
pub const CAP_FEEDBACK: &str = "feedback";
/// The capability that gives every agent of a chain `ask_decider`, a decision model asked about what the agent gathered (ADR-0118).
pub const CAP_JUDGMENT: &str = "judgment";
pub const CAP_EVENTS: &str = "events";
pub const CAP_EXTENSIONS: &str = "extensions";
/// The capability that opens Claude Code's Bash (policy.rs holds it back without).
pub const CAP_SHELL: &str = "shell";
/// The capability that opens Claude Code's WebFetch and WebSearch.
pub const CAP_WEB: &str = "web";
pub const CAP_OBJECTIVES: &str = "objectives";
pub const CAP_OKR: &str = "okr";
pub const CAP_DUELS: &str = "duels";
pub const CAP_COACHING: &str = "coaching";
/// The capability that keeps the agents' memory: read each teaching, write what it teaches (ADR-0117, librarian.rs in crew).
pub const CAP_MEMORY_KEEPING: &str = "memory_keeping";
/// The capability a trial of the judges plants its fault with (ADR-0102, seeded.rs in shipping), bound to its kind.
pub const CAP_SEEDING: &str = "seeding";

/// How a tool of this server is named in a call event: `mcp__<access::MCP_SERVER_NAME>__<tool>`.
pub const SOURCE_TOOL_PREFIX: &str = "mcp__remit__";

/// The capability whose agents ask for credentials, grants and protections of sources (capabilities.rs).
pub const CAP_SOURCES: &str = "sources";

/// The capability a chain holds to watch a signal (capabilities.rs).
pub const CAP_SIGNALS: &str = "signals";
