//! The ports orch drives the crates it does not own through, each declared here by its consumer with only what orch
//! calls: the sources (`source::Set`), the plugin host and the bundled plugins. Each crate's own type implements its
//! port beside it, so a test hands orch a fake and the server hands it the real one.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use wire::app::{ConnectionView, ToolSource};

use async_trait::async_trait;
use grant::Effect;
use source::{Caller, Extension, ExtensionTool, PluginRef, PollResult, ScriptInput, Status};

/// What agents read of the outside world, by capability (sources.rs): `source::Set`.
#[async_trait]
pub trait Sources: Send + Sync {
    /// The ids of every source Remit knows, the extensions' included.
    fn source_ids(&self) -> Vec<String>;
    /// The connection's credentials lent for one call of a plugin, through the gate (a plugin's sense, process or
    /// account call).
    async fn lend(
        &self,
        caller: &Caller,
        source: &str,
        connection: &str,
        call: &str,
        effect: Effect,
    ) -> source::Result<(PluginRef, BTreeMap<String, String>)>;
    /// Makes the sources of the extensions the owner applied, kept under root, the ones served.
    fn use_extensions(&self, root: &Path, exts: &[Extension]);
    /// Every source with its tools and connections, as it stands now.
    async fn catalog(&self) -> source::Result<Vec<ToolSource>>;
    /// A source with no connection yet, as the owner is asked to fill it.
    fn blank(&self, source: &str) -> Option<(ToolSource, ConnectionView)>;
    /// Whether each connection answers.
    async fn status(&self) -> source::Result<Vec<Status>>;
    /// Forgets the open client of a connection whose credentials changed.
    fn invalidate(&self, source: &str, connection: &str);
    /// The tools of the capabilities caps that some connection can serve.
    async fn ready_tools(&self, caps: &[&str]) -> source::Result<Vec<String>>;
    /// What a card's activity says while tool runs.
    fn doing(&self, tool: &str) -> String;
    /// The tools of the sources under caps, for a card's server: each called through the gate (sources.rs). A fake
    /// that holds no source serves none.
    fn serve(&self, _caps: &[&str]) -> Vec<source::ServedTool> {
        Vec::new()
    }
    /// Runs a script of a source's connection for Remit, through the gate, and answers what it printed (sensor.rs).
    async fn run_script(
        &self,
        caller: &Caller,
        source: &str,
        connection: &str,
        input: ScriptInput,
    ) -> source::Result<String>;
    /// How a step of a repository's release reaches a connection through the injecting proxy (ADR-0116): its address,
    /// credential and scrub, held in this process. A fake that holds no source reaches none.
    async fn injection(&self, source: &str, _connection: &str) -> source::Result<source::InjectSpec> {
        Err(source::Error::msg(format!("no source {source} on this server")))
    }
    /// Reads a page of a source for a sensor, through the gate (sensor.rs).
    async fn poll(
        &self,
        caller: &Caller,
        source: &str,
        connection: &str,
        query: &str,
        cursor: &str,
        limit: usize,
    ) -> source::Result<PollResult>;
    /// What the outside MCP server behind a connection lists now, which it opens to ask (mcp_servers.rs). A fake that holds
    /// no MCP server has none.
    async fn mcp_listing(&self, source: &str, _connection: &str) -> source::Result<Vec<ExtensionTool>> {
        Err(source::Error::msg(format!("{source} is not an MCP server")))
    }
}

#[async_trait]
impl Sources for source::Set {
    fn source_ids(&self) -> Vec<String> {
        source::Set::source_ids(self)
    }
    async fn lend(
        &self,
        caller: &Caller,
        source: &str,
        connection: &str,
        call: &str,
        effect: Effect,
    ) -> source::Result<(PluginRef, BTreeMap<String, String>)> {
        source::Set::lend(self, caller, source, connection, call, effect).await
    }
    fn use_extensions(&self, root: &Path, exts: &[Extension]) {
        source::Set::use_extensions(self, root, exts);
    }
    async fn catalog(&self) -> source::Result<Vec<ToolSource>> {
        source::Set::catalog(self).await
    }
    fn blank(&self, source: &str) -> Option<(ToolSource, ConnectionView)> {
        source::Set::blank(self, source)
    }
    async fn status(&self) -> source::Result<Vec<Status>> {
        source::Set::status(self).await
    }
    fn invalidate(&self, source: &str, connection: &str) {
        source::Set::invalidate(self, source, connection);
    }
    async fn ready_tools(&self, caps: &[&str]) -> source::Result<Vec<String>> {
        source::Set::ready_tools(self, caps).await
    }
    fn doing(&self, tool: &str) -> String {
        source::Set::doing(self, tool)
    }
    fn serve(&self, caps: &[&str]) -> Vec<source::ServedTool> {
        source::Set::serve(self, caps)
    }
    async fn run_script(
        &self,
        caller: &Caller,
        source: &str,
        connection: &str,
        input: ScriptInput,
    ) -> source::Result<String> {
        source::Set::run_script(self, caller, source, connection, input).await
    }
    async fn poll(
        &self,
        caller: &Caller,
        source: &str,
        connection: &str,
        query: &str,
        cursor: &str,
        limit: usize,
    ) -> source::Result<PollResult> {
        source::Set::poll(self, caller, source, connection, query, cursor, limit).await
    }
    async fn mcp_listing(&self, source: &str, connection: &str) -> source::Result<Vec<ExtensionTool>> {
        source::Set::mcp_listing(self, source, connection).await
    }
    async fn injection(&self, source: &str, connection: &str) -> source::Result<source::InjectSpec> {
        source::Set::injection(self, source, connection).await
    }
}

/// The programs of the extensions the owner applied, which run while agents use them (plugins.rs): each is a
/// `plugin_sdk::Plugin` (crates/plugin's `RemotePlugin`), started on its first call.
pub trait PluginHost: Send + Sync {
    /// The plugin spec runs, started on its first call and again on the first call after it died.
    fn plugin(&self, spec: &plugin::Spec) -> Arc<dyn plugin_sdk::Plugin>;
    /// Ends the process of plugin id, if it runs: its extension was replaced, disabled or deleted.
    fn stop(&self, id: &str);
    /// Ends every plugin's process: the server stops.
    fn close(&self);
}

/// The plugins that ship inside Remit (crates/bundled), which the owner need not apply.
pub trait Bundled: Send + Sync {
    /// The bundled plugin id names.
    fn lookup(&self, id: &str) -> Option<Arc<dyn plugin_sdk::Plugin>>;
    /// The ids of every bundled plugin.
    fn ids(&self) -> Vec<String>;
}

/// The machine Remit runs on, as the capacity reads it (admit.rs's `Pressure`).
pub trait Host: Send + Sync {
    /// The host now; none when it cannot be read. It may take a while: the caller reads it off the async threads.
    fn reading(&self) -> Option<HostReading>;
}

/// What the host had at one look.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HostReading {
    /// Bytes of memory free for a new process.
    pub free_memory: u64,
    /// Bytes resident in the processes Remit started (the agents' sessions and what they run).
    pub sessions_memory: u64,
    /// The load average over the last minute.
    pub load: f64,
    /// The cores the host has.
    pub cores: u32,
}
