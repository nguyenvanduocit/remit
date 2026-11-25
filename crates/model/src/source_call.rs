use serde::{Deserialize, Serialize};

use crate::Time;

/// One call an agent made to the outside world through a source (ADR-0018, invariant 3), kept whatever its outcome. It
/// names who asked what of which connection and holds what the input was without a secret: a program's argv, a script's
/// language, size and hash. Nothing in it is a value of a connection.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SourceCall {
    pub chain: String,
    pub agent: String,
    pub source: String,
    pub connection: String,
    pub tool: String,
    /// Empty when the call failed before the tool was found.
    pub effect: String,
    /// tool, env, proxy or poll.
    pub form: String,
    /// What the input was, without its secrets.
    pub detail: String,
    /// ok, denied or error.
    pub outcome: String,
    /// When the call was turned away before it ran: no_grant or rail.
    pub refusal: String,
    pub at: Time,
}
