//! The records Remit stores and sends to its channels, all of them exported from the crate root.
//!
//! Every stored document must still read (ADR-0072, section 5), and a record crosses the link as postcard (ADR-0107), so
//! the records follow one set of rules:
//!
//! - Every field is written. A field whose value may be absent is an `Option`, left out when it is none: the one field
//!   left out of a document, and postcard reads it back as none.
//! - A key a document lacks reads as the field's default (each record is `#[serde(default)]`): a document stored before
//!   a field existed has no key for it. A key no field names is ignored. A list stored as `null` or a time stored as the
//!   zero time by an earlier build was removed by migration 0110, so neither is read.
//! - A string enum is a Rust enum made by `text_enum!`, which reads only the values it names: a field whose value may be
//!   absent is an `Option` of it. A document holding a value this build does not name fails to read, and the store
//!   leaves that record out of what it reads, untouched, and counts it (ADR-0107).
//! - A time that may be unset is an `Option<Time>`. Times, durations and JSON another system defines have one form
//!   ([`stored_json`]); a document is written by `serde_json`.

mod account;
mod actor;
mod change;
mod checks;
mod client_key;
mod cut;
mod duel;
mod employee;
mod event;
mod event_status;
mod extension;
mod feedback;
mod group;
mod image;
mod inbox;
mod invalid;
mod judgment;
mod landing;
mod library;
mod memory;
mod mentor;
mod model;
mod objective;
mod outcome;
mod owner_words;
mod project;
mod replay;
mod retrospective;
mod round;
mod second_opinion;
mod seeded;
mod seen;
mod sensor;
mod settings;
mod signal;
mod source_call;
pub mod stored_json;
pub mod tagged;
mod text_enum;
mod worker;
mod workflow;

pub use account::*;
pub use actor::*;
pub use change::*;
pub use checks::*;
pub use client_key::*;
pub use cut::*;
pub use duel::*;
pub use employee::*;
pub use event::*;
pub use event_status::*;
pub use extension::*;
pub use feedback::*;
pub use group::*;
pub use image::*;
pub use inbox::*;
pub use invalid::Invalid;
pub use judgment::*;
pub use landing::*;
pub use library::*;
pub use memory::*;
pub use mentor::*;
pub use model::*;
pub use objective::*;
pub use outcome::*;
pub use owner_words::*;
pub use project::*;
pub use replay::*;
pub use retrospective::*;
pub use round::*;
pub use second_opinion::*;
pub use seeded::*;
pub use seen::*;
pub use sensor::*;
pub use settings::*;
pub use signal::*;
pub use source_call::*;
pub use stored_json::{Duration, ForeignJson, Time};
pub use text_enum::UnknownValue;
pub use worker::*;
pub use workflow::*;

// What `text_enum!`, `json_tagged!` and `json_paired!` expand to names through this crate, so a crate that declares one
// needs none of these as a dependency.
#[doc(hidden)]
pub use {schemars as __schemars, serde as __serde, serde_json as __serde_json};
