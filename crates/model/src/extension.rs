use serde::{Deserialize, Serialize};

use crate::text_enum;
use crate::{Actor, Time};

/// An extension the owner applied (docs/remit-design.md, K5): its manifest as JSON, the document the source crate reads,
/// kept as the owner applied it. The program it names is a file under the extensions directory, pinned by the hash in the
/// manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StoredExtension {
    pub id: String,
    pub doc: String,
    pub applied_at: Time,
}

/// The owner's word that an extension, one that ships with Remit or one he applied, is not a source for now: its tools
/// are not offered to agents until he enables it. `by` says who disabled it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DisabledExtension {
    pub id: String,
    pub by: Actor,
    pub at: Time,
}

/// A plugin source as the owner sees it: what it is, where it came from and whether he disabled it. It is a view over the
/// bundled plugins, the extensions he applied and the ones he disabled, and its JSON is the contract's type of the same
/// name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Extension {
    pub id: String,
    pub label: String,
    pub capability: String,
    pub origin: ExtensionOrigin,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_by: Option<Actor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_at: Option<Time>,
    pub tools: Vec<ExtensionTool>,
    pub poll: bool,
    pub ui: bool,
    pub sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applied_at: Option<Time>,
}

text_enum! {
    /// How a plugin comes to the server.
    #[derive(Default)]
    pub enum ExtensionOrigin {
        /// Ships in Remit's own binary.
        #[default]
        Bundled = "bundled",
        /// A binary the owner applied.
        Program = "program",
        /// An API behind the injecting proxy.
        Api = "api",
        /// An outside MCP server Remit is the client of (ADR-0077).
        Mcp = "mcp",
    }
}

/// One tool of a plugin: its name, what it does, the grant a chain needs to call it and the inputs it takes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ExtensionTool {
    pub name: String,
    pub what: String,
    pub effect: String,
    pub inputs: Vec<ExtensionToolInput>,
}

/// One top-level input of a tool's input schema: `type` as the schema names it ("array of string" for a list), `choices`
/// the values its enum allows, `default` its default as JSON.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ExtensionToolInput {
    pub name: String,
    pub r#type: String,
    pub what: String,
    pub required: bool,
    pub choices: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}
