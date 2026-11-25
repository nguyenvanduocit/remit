//! Runs the plugins the owner applied (ADR-0034, ADR-0072 decision 7): a `RemotePlugin` is one program, pinned by its hash,
//! run as a child process that speaks `link` on its stdio, kept while it is used and started again by the next call after it
//! dies. It implements `plugin_sdk::Plugin`, the trait a plugin that ships with Remit implements in-process.

mod remote;
mod start;

pub use remote::{RemotePlugin, Spec, Stopped};
