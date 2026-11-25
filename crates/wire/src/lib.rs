//! The one definition of what crosses the wire, compiled into the server and every client core (ADR-0072 decision 2).
//! Each module is a contract: a capability set of the link, written with `link::contract!`. `app`: the web app, the
//! owner's and a mentor's (ADR-0089). `runtime`: the core and its AI runtime, both ways. `plugin`: the host and an
//! applied extension (decision 7).

use serde::{Deserialize, Serialize};

pub mod app;
pub mod plugin;
pub mod runtime;

/// The answer to a call that has nothing to return but "done"; a failure is a `link::Failure`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Ack {}
