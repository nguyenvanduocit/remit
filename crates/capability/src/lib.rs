//! What an agent is given (ADR-0096): agents, playbooks, capabilities and their requests, the kinds of chain, the skill
//! library, and the rails on what an agent writes.

pub mod accept;
pub mod account_tools;
pub mod agent_tools;
pub mod agents;
pub mod automation_tools;
pub mod briefs;
pub mod capabilities;
pub mod capability_request;
pub mod chainasks;
pub mod change_tools;
pub mod gate_instructions;
pub mod judge_store;
pub mod kinds;
pub mod library;
pub mod library_import;
pub mod library_render;
pub mod library_tools;
pub mod my_work_tools;
pub mod notify;
pub mod okr;
pub mod over;
pub mod playbook_tools;
pub mod playbooks;
pub mod plugin_ships;
pub mod rail;
mod service;

pub use service::Capabilities;
