//! What a Remit plugin is written with, and the one contract every runner of it keeps (ADR-0072 decision 7). A plugin is a
//! [`Plugin`]: most are a [`Definition`], a plugin written as data (its tools, whether it is a source of events, the kinds of
//! sensor and processors it adds to the signal pipeline, whether it provides Claude accounts, the frontend it brings). A
//! plugin that ships with Remit is called in-process; an applied extension is a program whose `main` calls [`serve`], and
//! Remit calls it over `link` on its stdio through an adapter that implements the same trait.
//!
//! ```ignore
//! fn main() -> std::process::ExitCode {
//!     plugin_sdk::serve(plugin_sdk::Definition {
//!         label: "Example".into(),
//!         capability: "data".into(),
//!         vars: vec![plugin_sdk::Var { name: "EXAMPLE_TOKEN".into(), what: "an API token".into(), secret: true, ..Default::default() }],
//!         tools: vec![plugin_sdk::Tool::new("list", "List things.", plugin_sdk::Effect::Read, list)],
//!         ..Default::default()
//!     })
//! }
//! ```

mod definition;
pub mod pipe;
mod plugin;
mod sense;
mod serve;

pub use definition::{Account, Call, Definition, Error, Invalid, Poll, PollQuery, Tool};
pub use plugin::{Asset, Forge, Plugin, Sensed, Senses};
pub use sense::{Emitter, ProcessInput, ProcessedReading, Processor, Sensing, SensorKind};
pub use serve::serve;
pub use wire::plugin::{
    Agent, Auth, Effect, Firing, Manifest, PollAnswer, PollAsk, PollItem, ProcessAnswer, Reading, SenseOpen, Signal,
    Slot, Subscription, Ui, Var, Vars,
};
