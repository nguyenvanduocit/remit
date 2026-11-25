use serde::{Deserialize, Serialize};

use crate::{Actor, Time};

/// A way in the owner opened for one outside agent: a key only its holder presents, and the limits the owner set on what
/// that agent may start. It is made and changed by telling the Assistant. The key is no part of this record, which every
/// channel and every agent may read: the server keeps it sealed apart from it and shows it only to the owner, when he
/// asks.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClientKey {
    pub id: String,
    pub name: String,
    /// What the agent that holds it is for, in the owner's words.
    pub note: String,
    /// How many `work` chains it may start in a day (the server's local date); zero starts none.
    pub chains_per_day: i64,
    /// How many acts that start or steer it may make in any hour.
    pub actions_per_hour: i64,
    /// A key that opens nothing until it is resumed.
    pub paused: bool,
    /// When and from where it last opened the endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<Time>,
    pub last_addr: String,
    /// Who made it or last changed its settings.
    pub by: Actor,
    pub created_at: Time,
    pub updated_at: Time,
    /// Counts its stored changes.
    pub rev: i64,
}
