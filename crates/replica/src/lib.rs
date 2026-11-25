//! The client core (ADR-0072 decision 10): one link session to the server and the replica of what the server pushes,
//! with no I/O. A shell (the browser's `client-web`) feeds `Replica::handle` what its transport saw, the time and the
//! owner's asks, and carries out the `Output`s on its transport and timer. The core keeps the records and hands the
//! shell the models it shows (ADR-0095 decision 5): whole when shown, then the rows that changed, with how fresh each
//! is. It holds what the server answered and pushed; it decides only which copy is the newer, where a record goes in
//! its model, and the order its own calls go in. It builds for wasm32 and natively.

mod archived;
mod assistant;
mod banner;
mod calls;
mod catalog;
mod core;
mod drafts;
mod event;
mod events;
mod fresh;
mod held;
mod index;
mod io;
mod models;
mod opened;
mod outbox;
mod records;
mod repos;
mod shell;
mod tally;

pub use crate::core::{Options, Replica};
pub use crate::io::{Input, Output, Tab};
pub use crate::shell::{
    Ask, Banner, BannerAction, BannerTone, Change, Freshness, Kept, KeptDraft, KeptSend, Key, Model, Notice, Pending,
    PendingStatus, Phase, Retry, Row, StaleReason, State, Ticket, Told, What,
};
