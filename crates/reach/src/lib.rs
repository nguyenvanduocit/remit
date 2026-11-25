//! How agents reach the outside world (ADR-0096): sources, credentials, grants and the gate, programs and scripts,
//! plugins and extensions, outside MCP servers.

pub mod credentials;
pub mod extension_state;
pub mod extensions;
pub mod forge_service;
pub mod grant_access;
pub mod grant_tools;
pub mod grants;
pub mod mcp_servers;
pub mod plugins;
pub mod program;
pub mod release_step;
pub mod script;
mod service;
pub mod source_calls;
pub mod source_store;
pub mod source_tools;
pub mod sources;

pub use service::Reach;
