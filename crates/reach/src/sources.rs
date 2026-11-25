//! The tools through which agents read the outside world (ADR-0015) are the source crate's. A chain holds the capability
//! that names what it may read (data, observe, customers); every agent of such a chain gets the tools of the
//! sources under it, the goal judge included, so a claim about stored values or a running system is checked, not taken on
//! trust. Credentials stay in the backend: the owner's stored values (source_store.rs), and for the default connection the
//! process environment; a source that lacks them is still offered and its tools say what to set.
//!
//! The source crate's `Set` reads through ports the orchestrator implements (its `Store`, `Policy`, `Recorder` and
//! `Plugins`), and the orchestrator holds the `Set`: `OrchPorts` is made first, hands the `Set` its ports, and is bound to
//! the orchestrator once that is made. A call before then, or after the orchestrator is dropped, is refused.

use std::sync::{Arc, OnceLock, Weak};
use wire::app::ToolSource;

use model::{Card, Chain};
use source::{BoxError, CAP_CUSTOMERS, CAP_DATA, CAP_OBSERVE, Caller, Status};

use crate::Reach;
use chains::cap_names::CAP_SOURCES;
use chains::tools::{Answer, Effect, Tools};
use chains::{Error, Result};

/// The orchestrator as the ports it implements reach it: bound once it is made.
#[derive(Clone, Default)]
pub(crate) struct Bound(Arc<OnceLock<Weak<Reach>>>);

impl Bound {
    /// The orchestrator, or why a call cannot reach it.
    pub(crate) fn get(&self) -> std::result::Result<Arc<Reach>, BoxError> {
        self.0
            .get()
            .ok_or("the orchestrator is not made yet")?
            .upgrade()
            .ok_or_else(|| "the orchestrator has stopped".into())
    }
}

/// The ports the source crate's `Set` is opened with, each the orchestrator's: `bind` it to the orchestrator the `Set` is
/// then given to.
#[derive(Clone, Default)]
pub struct OrchPorts(Bound);

impl OrchPorts {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The owner's stored credentials (source_store.rs).
    #[must_use]
    pub fn store(&self) -> Arc<dyn source::Store> {
        Arc::new(crate::source_store::SourceStore(self.0.clone()))
    }

    /// The grants every call passes (grants.rs).
    #[must_use]
    pub fn policy(&self) -> Arc<dyn source::Policy> {
        Arc::new(crate::grants::GrantPolicy(self.0.clone()))
    }

    /// The account of calls (source_calls.rs).
    #[must_use]
    pub fn recorder(&self) -> Arc<dyn source::Recorder> {
        Arc::new(crate::source_calls::CallAccount(self.0.clone()))
    }

    /// Where the extensions' programs run (plugins.rs).
    #[must_use]
    pub fn plugins(&self) -> Arc<dyn source::Plugins> {
        Arc::new(crate::plugins::PluginCalls(self.0.clone()))
    }

    /// Where what an outside MCP server lists is pinned (mcp_servers.rs).
    #[must_use]
    pub fn pins(&self) -> Arc<dyn source::Pins> {
        Arc::new(crate::mcp_servers::McpPins(self.0.clone()))
    }

    /// What a bundled plugin granted `forge.read` asks the forge (forge_service.rs).
    #[must_use]
    pub fn forge(&self) -> Arc<dyn plugin_sdk::Forge> {
        Arc::new(crate::forge_service::ForgeLent(self.0.clone()))
    }

    /// Makes o the orchestrator the ports reach. Binding again changes nothing: the first stays.
    pub fn bind(&self, o: &Arc<Reach>) {
        if (self.0).0.set(Arc::downgrade(o)).is_err() {
            tracing::warn!("the sources' ports were bound already: the first orchestrator stays");
        }
    }
}

impl Reach {
    /// What an agent calling this server's tool is doing: the line Remit's own tools carry, else the one the source that
    /// owns the tool gives, an extension's included.
    pub fn tool_doing_of(&self, name: &str) -> String {
        chains::activity::tool_doing(name).map_or_else(|| self.sources.doing(name), str::to_owned)
    }

    /// Gives t, the tools of card, the tools of the sources under the capabilities chain ch holds, and list_sources, which
    /// names their connections, so an agent chooses the one a tool reads. Every call carries who makes it, from the card and
    /// never from the call's input, for the gate to name to the grant policy.
    pub fn add_source_tools(self: &Arc<Self>, t: &mut Tools, card: &Card, ch: &Chain) {
        let caller = Caller {
            chain: ch.id.clone(),
            agent: card.id.clone(),
        };
        let caps: Vec<&str> = ch.capabilities.iter().map(String::as_str).collect();
        for st in self.sources.serve(&caps) {
            let schema = match st.input_schema.clone() {
                serde_json::Value::Object(m) => m,
                _ => serde_json::Map::new(),
            };
            let (name, description) = (st.name.clone(), st.description.clone());
            let caller = caller.clone();
            let effect = if st.read_only { Effect::Reads } else { Effect::Writes };
            t.serving(self)
                .add_schema(name, effect, description, schema, move |_o, args| -> Answer {
                    let (st, caller) = (st.clone(), caller.clone());
                    Box::pin(async move { st.call(caller, args).await.map_err(|e| Error::refused(e.to_string())) })
                });
        }
        if ch
            .capabilities
            .iter()
            .any(|c| [CAP_DATA, CAP_OBSERVE, CAP_CUSTOMERS, CAP_SOURCES].contains(&c.as_str()))
        {
            self.add_source_list_tool(t, caller);
        }
    }

    /// Which connection of which source can be read and which variables the others lack, for the server to log at start.
    pub async fn source_status(&self) -> Result<Vec<Status>> {
        self.sources.status().await.map_err(|e| Error::refused(e.to_string()))
    }

    /// The sources as the channels show them: each with its tools and its connections.
    pub async fn source_entries(&self) -> Result<Vec<ToolSource>> {
        self.sources.catalog().await.map_err(|e| Error::refused(e.to_string()))
    }

    /// The data tools chain ch can query with: none unless it holds the data capability, and only those of a
    /// source that has a configured connection.
    pub async fn data_tools(&self, ch: &Chain) -> Result<Vec<String>> {
        if !ch.capabilities.iter().any(|c| c == CAP_DATA) {
            return Ok(Vec::new());
        }
        self.sources
            .ready_tools(&[CAP_DATA])
            .await
            .map_err(|e| Error::refused(e.to_string()))
    }
}
