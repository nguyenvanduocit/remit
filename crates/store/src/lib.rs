//! Remit's records in one SQLite file, one module per kind of record (`Tx::put_chain` writes a chain, `Reader::chains`
//! reads them).
//!
//! The file carries its migrations with their checksums (`migrations/NNNN.sql`, ADR-0072 section 6), its pragmas and its
//! lock, and its documents in the stored JSON shape (`model::stored_json`). Writes go to one connection owned by a thread
//! of its own (`Store::update`); reads run on a fixed set of `query_only` connections on blocking threads
//! (`Store::read`), each in one read transaction.

mod accounts;
mod catalog;
mod changes;
mod client_key;
mod duels;
mod employees;
mod event_signal;
mod events;
mod export;
mod extensions;
mod feedback;
mod grant;
mod homes;
mod images;
mod judgments;
mod landings;
mod library;
mod library_secrets;
mod lock;
mod memories;
mod memory_daemon;
mod mentors;
mod migrate;
mod migrations;
mod objectives;
mod pack;
mod ping;
mod projects;
mod restore;
mod retrospectives;
mod rounds;
mod second_opinions;
mod seeded;
mod seen;
mod sensor;
mod settings;
mod signal;
mod snapshot;
mod source;
mod source_calls;
mod store;
mod telemetry;
mod worker;
mod workflows;

#[cfg(feature = "testing")]
pub mod storetest;

pub use catalog::{Catalog, Moved, Record};
pub use export::{Exported, export};
pub use homes::{HOME_RECORDS, KEPT_PAST_HOME};
pub use judgments::{JudgmentScore, JudgmentTally};
pub use landings::CarriedMr;
pub use memories::{ABOUT, MemoryEdge, MemoryReader, passage_hash};
pub use memory_daemon::{DaemonMemory, read_memory_daemon};
pub use restore::{Restored, restore};
pub use signal::{FiringFilter, SignalRow};
pub use snapshot::{Snapshot, schema_version, snapshot_before_update, snapshot_dir, snapshots};
pub use source::SourceValue;
pub use store::{Error, Reader, Result, Store, Tx};
pub use telemetry::{Telemetry, TelemetryError};
