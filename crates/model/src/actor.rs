use serde::{Deserialize, Serialize};

use crate::text_enum;

text_enum! {
    /// Who acts: the owner, an agent, an outside agent holding a client key, or Remit itself.
    #[derive(Default)]
    pub enum ActorKind {
        /// The owner, through a channel.
        Owner = "owner",
        /// An agent of a chain.
        Agent = "agent",
        /// The Assistant, acting on the owner's words.
        Assistant = "assistant",
        /// Remit itself: its scheduler, its rails, a restart.
        #[default]
        Remit = "remit",
        /// An outside agent acting through a client key; never the owner.
        Client = "client",
        /// A mentor, on the card they help (ADR-0062).
        Mentor = "mentor",
    }
}

/// Who did something. The owner is named by the account his channel signed him in with and the channel he acted
/// through; an agent and the Assistant by their card id; Remit by the part of it that acted, when that matters. Rules on
/// who may do what read `kind`, never a name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Actor {
    pub kind: ActorKind,
    pub who: String,
    pub via: String,
}

fn actor(kind: ActorKind, who: &str, via: &str) -> Actor {
    Actor {
        kind,
        who: who.to_owned(),
        via: via.to_owned(),
    }
}

/// The owner, signed in as who, acting through channel via.
pub fn by_owner(who: &str, via: &str) -> Actor {
    actor(ActorKind::Owner, who, via)
}

/// The agent of card card_id.
pub fn by_agent(card_id: &str) -> Actor {
    actor(ActorKind::Agent, card_id, "")
}

/// The mentor of help help_id, on the card they help: their message steers that card's session and is recorded as
/// theirs (ADR-0062).
pub fn by_mentor(help_id: &str) -> Actor {
    actor(ActorKind::Mentor, help_id, "")
}

/// The Assistant, whose card is card_id, acting on the owner's words.
pub fn by_assistant(card_id: &str) -> Actor {
    actor(ActorKind::Assistant, card_id, "")
}

/// The outside agent that holds client key key_id, which reached Remit through its MCP endpoint.
pub fn by_client(key_id: &str) -> Actor {
    actor(ActorKind::Client, key_id, "mcp")
}

/// Remit itself; part names which part of it, when anything tells parts apart.
pub fn by_remit(part: &str) -> Actor {
    actor(ActorKind::Remit, part, "")
}

impl Actor {
    /// Whether the actor acts as the owner: the owner himself, or the Assistant on his words. The powers the owner has
    /// over chains (his messages read first, reopening a finished chain) are his whichever of the two acts.
    pub fn for_owner(&self) -> bool {
        matches!(self.kind, ActorKind::Owner | ActorKind::Assistant)
    }
}
