use serde::{Deserialize, Serialize};

use crate::text_enum;
use crate::{Actor, Time};

text_enum! {
    /// Whether a project is offered to the agents that choose a chain's project.
    #[derive(Default)]
    pub enum ProjectStatus {
        /// Listed to the Chairs, and its chains read it.
        #[default]
        Active = "active",
        /// Offered to no one; still read on the objectives that name it, so no reference dangles.
        Archived = "archived",
    }
}

/// A connection of a source that is a project's own: Datadog's `prod-a`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ConnectionRef {
    pub source: String,
    pub connection: String,
}

/// One product the owner's work is on, the durable context its objectives belong to (ADR-0108). A project is context:
/// it grants, narrows and limits nothing. It is written over the `rev` it was read at, only by the Assistant on the
/// owner's words; a chain's project is its objective's, never stored on the chain.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub rev: i64,
    /// Unique among projects.
    pub name: String,
    pub status: ProjectStatus,
    /// Who the product is for, in words: where a Chair's CUSTOMER section starts (ADR-0016).
    pub customer: String,
    /// The owner's notes on the product (what it is, where it runs, what matters), in Markdown. What a repository says
    /// of itself stays in the repository.
    pub brief: String,
    /// Its repositories' remotes, `host/group/project`.
    pub repos: Vec<String>,
    pub connections: Vec<ConnectionRef>,
    /// Who made it or last changed it.
    pub by: Actor,
    pub created_at: Time,
    pub updated_at: Time,
}
