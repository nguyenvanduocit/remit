//! The client core in the browser (ADR-0072 decision 10): `replica` over `link`, bound with wasm-bindgen. One `Core`
//! serves every tab of the browser from a SharedWorker (ADR-0095 decision 4, `Tabs`, `worker.rs`): it takes each tab's
//! asks (`Ask`), runs them through `Replica::handle` with the clock, carries out the transport ops on the browser's
//! WebTransport and its one timer on `setTimeout`, and hands each tab what it has to show (`Told`: the models it shows
//! and their changes, state, answers, notices), one batch per step, on its port. Both cross as plain objects by
//! `serde-wasm-bindgen` (decision 5); `useModel` in `web/src/core/` applies a model's changes to its rows, and nothing
//! here decides anything the core does not.
//!
//! With the `fake` feature the module also carries `FakeServer`, the server's half of a link session answering from
//! what the page or a test hands it, so the real core runs where there is no WebTransport: in the page itself (vitest in
//! Node, `Core::fake` and `Core::tab`) or in the verify pages' SharedWorker (`Tabs::fake`).

// The binding has a transport only in the browser or with the fake: a native build without `fake` holds only what a
// tab says on its port.
#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(feature = "fake")]
mod fake;
#[cfg(any(target_arch = "wasm32", feature = "fake"))]
mod js;
mod port;
#[cfg(any(target_arch = "wasm32", feature = "fake"))]
mod shell;
#[cfg(target_arch = "wasm32")]
mod worker;

#[cfg(feature = "fake")]
pub use crate::fake::FakeServer;
pub use crate::port::FromTab;
#[cfg(any(target_arch = "wasm32", feature = "fake"))]
pub use crate::shell::{Core, Tab};
#[cfg(target_arch = "wasm32")]
pub use crate::worker::Tabs;
