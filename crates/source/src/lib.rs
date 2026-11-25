//! The tools through which agents read the outside world: stored data, metrics, incidents, customer conversations. Why they
//! are Remit's own, and the threats they answer, is ADR-0015.
//!
//! A source is a `Provider`: the credentials it needs (`missing`), every variable it reads (`vars`) and the tools it offers
//! (`tools`). One Provider is one connection of the source: the same code with another set of credentials reaches another
//! place, so the owner may name any number of them (connection.rs). A tool takes the connection it reads as the
//! `connection` argument, which the kernel adds to the tool's schema; a source's own input types never carry it. Every tool
//! reads and nothing else: each source enforces that in its own code (the guards in sqlguard.rs, mongoguard.rs and the
//! Bytebase guards, the verb and path rules of `Api`), never by trusting the credential's rights or the agent's promise.
//!
//! Every call an agent makes, and every poll Remit makes, passes the gate (gate.rs, ADR-0018): it resolves the connection,
//! asks the `Policy` whether the `Caller` may take the `Action`, runs the act under a deadline, removes secrets, heads the
//! answer with `connection: <source>/<name>` and cuts it, then tells the `Recorder` once, whatever the outcome. With no
//! policy a read by a typed tool passes and anything else is refused.
//!
//! A source is also reached as a program (form env, program.rs) when it answers `Provider::program`, as a script behind the
//! injecting proxy (form proxy, script.rs) when it answers `Provider::inject`, and is polled by Remit (poll.rs) when it
//! answers `Provider::poller`. An extension (extension.rs) is a source a plugin brings, applied by the owner as a manifest, or
//! an outside MCP server Remit is the client of (mcp.rs, ADR-0077).

mod api;
mod bytebase;
mod cloudwatch;
mod connection;
mod coroot;
mod datadog;
mod env;
mod error;
mod extension;
mod gate;
mod grafana;
mod lend;
mod limit;
mod mcp;
mod mongo;
mod mongoguard;
mod null_json;
mod poll;
mod program;
mod reading;
mod redact;
mod script;
mod source;
mod sqlguard;
mod starrocks;
mod text;
mod when;

pub use api::{Api, Http, Query, encode_query, http_client, query};
pub use connection::{ConnectionInfo, DEFAULT_CONNECTION, Options, ServedTool, Set, Status, Store};
pub use datadog::site as datadog_site;
pub use env::{Clock, Env, load_env, parse_env, vault_files};
pub use error::{BoxError, BoxFuture, Error, Result, StatusError};
pub use extension::{
    Extension, ExtensionAuth, ExtensionProcessor, ExtensionSensorKind, ExtensionSlot, ExtensionSubscription,
    ExtensionTool, ExtensionUi, PluginRef, Plugins,
};
pub use gate::{Action, Call, Caller, Form, Outcome, Policy, Recorder};
pub use lend::{ACCOUNT_CALL, PROCESS_CALL, SENSE_CALL};
pub use limit::Limit;
pub use mcp::{ExtensionMcp, Listed, McpTransport, Pins, held, pins_of};
pub use mongoguard::{check_mongo_filter, check_mongo_pipeline};
pub use null_json::null_as_zero;
pub use poll::{POLL_CALL, POLL_TEXT_MAX, PollItem, PollMark, PollResult, Pollable, rfc3339, rfc3339_nano};
pub use program::{Job, Output, PROGRAM_PATH, ProgramEnv, ProgramSpec, RUN_TOOL, RunInput, Runner, Spawned, Verify};
pub use reading::{READING_PREFIX, is_reading, reading_poll};
pub use redact::{
    REMOVED, clean_text, clean_texts, mask_emails, redact_text, redact_value, render, render_value, sensitive_key,
};
pub use script::{InjectJob, InjectSpec, SCRIPT_ENFORCED, SCRIPT_TOOL, ScriptInput, Scrub, Sign};
pub use source::{CAP_CUSTOMERS, CAP_DATA, CAP_OBSERVE, Info, Provider, Shared, Tool, Var, describe, new_tool};
pub use sqlguard::read_only_sql;
pub use text::{OUTPUT_MAX, cut_bytes, limit, quote_escaped, shorten, table, value_text_of};
pub use when::{duration_text, parse_when};
