//! Everything that comes into Remit is an event: a reading of an event signal (docs/adr/0024). The signal `event:<name>`
//! is what one outside system, script or agent says; its settings are an EventSignal, what arrived is the reading's
//! EventValue, and what became of it is the firings of the subscriptions to the key (ADR-0036). Whichever way an event
//! came in (the HTTP door, the CLI, a sensor's poll, an agent's tool) it is the same record.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::text_enum;
use crate::{Actor, SignalKey, Time};

/// Begins the key of every event signal's readings.
pub const EVENT_KEY_PREFIX: &str = "event:";

/// The key of the readings of event signal name: `event:<name>`.
pub fn event_key(name: &str) -> SignalKey {
    SignalKey(format!("{EVENT_KEY_PREFIX}{name}"))
}

text_enum! {
    /// The way an event came in.
    #[derive(Default)]
    pub enum EventVia {
        /// Posted to the HTTP door, /hooks/<name>.
        #[default]
        Door = "door",
        /// Posted to the door by `remit emit`.
        Cli = "cli",
        /// Read from a source by a sensor of the signal.
        Poll = "poll",
        /// Written by an agent's emit_event tool.
        Agent = "agent",
        /// Written by a processor of a plugin (ADR-0049).
        Plugin = "plugin",
    }
}

/// What arrived, as its writer sent it: no schema is asked of it. Remit bounds the body and keeps every header but the
/// credentials. It is the `event` of a reading's value. (The name says inbound because Event is a card's event, what its
/// session streamed.)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct InboundEvent {
    pub via: EventVia,
    /// Every header the writer set but the credentials.
    pub header: BTreeMap<String, String>,
    /// The URL's query, with the credential the sender may have put there taken out.
    pub query: String,
    pub body: String,
    /// Body is the start of a longer body: what Remit keeps of one is limited.
    pub body_cut: bool,
}

/// The value of a reading of an event signal: what arrived. A subscription's condition reads it as `value.event`, and
/// `body` and `header` are its short names for the event's own.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EventValue {
    pub event: InboundEvent,
}

/// Addresses one event: the reading of event signal `signal` at revision `rev`. Its text form, `event:<signal>#<rev>`, is
/// the address of the event's page and of the chain an event started.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventRef {
    pub signal: String,
    pub rev: i64,
}

impl fmt::Display for EventRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{EVENT_KEY_PREFIX}{}#{}", self.signal, self.rev)
    }
}

/// The event an address names; none for text that is no address.
pub fn parse_event_ref(s: &str) -> Option<EventRef> {
    let (signal, rev) = s.strip_prefix(EVENT_KEY_PREFIX)?.split_once('#')?;
    let rev = rev.parse::<i64>().ok()?;
    (!signal.is_empty() && rev >= 1).then(|| EventRef {
        signal: signal.to_owned(),
        rev,
    })
}

/// The settings of one event signal, `event:<name>`: what it is for and whether writers are let in. What fills it besides
/// its door, the CLI and agents is its sensors. Its name is the last part of its door's address (`/hooks/<name>`). It is
/// made and changed by telling the Assistant. The door's key is no part of this record, which every channel and every
/// agent may read: the server keeps it sealed apart from it and shows it only to the owner, when he asks.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EventSignal {
    pub id: String,
    pub name: String,
    /// What it is for, in the owner's words.
    pub note: String,
    /// A signal that ships with Remit, as a built-in playbook does: its settings start as the code gives them and follow
    /// Remit until the owner changes them, when his version is kept (`edited`) until he resets it. It is never deleted.
    pub builtin: bool,
    /// A builtin signal whose settings are the owner's version, not what ships.
    pub edited: bool,
    /// A signal that lets no writer in, whichever way it comes, until it is resumed.
    pub paused: bool,
    /// A signal whose events stay on this machine: no decider reads them, so a subscription's screen of it asks none
    /// (ADR-0118) and every event it lets through fires as it would with no screen.
    pub kept: bool,
    /// Who made it or last changed it.
    pub by: Actor,
    pub created_at: Time,
    pub updated_at: Time,
    /// Counts its stored changes.
    pub rev: i64,
}
