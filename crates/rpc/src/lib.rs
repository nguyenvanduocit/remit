//! The web app's adapter onto the orchestrator (ADR-0072 decisions 2, 3 and 12; ADR-0089): a session on the link that comes
//! in with the owner key is answered `wire::app`'s calls by `Api`, as that owner, and one that comes in with a mentor's
//! worker key by `MentorApi`, as that mentor; each is pushed the changes the orchestrator tells (`Broadcast`, its
//! `Notifier`) that reach it, as `wire::app::Push`es, each on its home's stream.
//!
//! What a binary does: makes a `Broadcast`, gives it to `orch::Deps::notify`, makes a `Server` over the orchestrator, and
//! hands it each session on the link whose hello asks for `wire::app::CAPABILITY` (`Server::serve`).

mod api;
mod broadcast;
pub mod convert;
mod link_report;
mod mentor_api;
mod pending;
mod serve;
pub mod session;

pub use api::Api;
pub use broadcast::{BEHIND_BY, Broadcast};
pub use mentor_api::MentorApi;
pub use serve::{BAD_TOKEN, CHANNEL, NEW_BUNDLE, Principal, Server, Verifier};
