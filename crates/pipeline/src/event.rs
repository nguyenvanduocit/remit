//! Everything that comes into Remit is an event of an event signal (ADR-0024, ADR-0036), and `emit` is the event signal's
//! adapter over ingest (ingest.rs), whoever writes it: the HTTP door (and `remit emit`, its client), the poller of a sensor,
//! an agent's emit_event tool and a plugin's processor. `emit` names the event, cuts its body to size and gives its
//! dedupes; ingest stores it before its writer is answered, as a reading of `event:<name>`, so none is lost to a restart,
//! and the subscriptions to that key act on it on the Tick as they do on any reading. What became of an event is the
//! firings of those subscriptions (event_record.rs).
//!
//! The rails stay code: the door authenticates the sender; `emit` refuses a signal no one made, and ingest one that is
//! paused, for every writer; what is kept is limited here.

use std::collections::BTreeMap;

use model::{Cause, EventValue, EventVia, ForeignJson, InboundEvent, Reading, Time, Writer, WriterKind, event_key};
use sha2::{Digest, Sha256};

use crate::event_record::event_record_of;
use crate::event_record::event_ref;
use crate::event_signal::event_value_of;
use crate::event_signal_keys::is_event_signal_name;
use crate::service::Pipeline;
use chains::ingest::Write;
use chains::{Error, Result};

/// How much of a body is kept and read: the start of a longer one, marked as cut.
const EVENT_BODY_MAX: usize = 64 << 10;
/// How long a retry of a write with no id of its own is told by its body: the same body after it is a new event (an
/// alert that fires again). A writer's own id is the same event for as long as the event is kept.
const EVENT_RETRY_WINDOW_MS: i128 = 10 * 60 * 1000;

/// One write of an event, as its writer hands it to `emit`.
#[derive(Debug, Clone, Default)]
pub struct Emission {
    /// The name of the event signal.
    pub signal: String,
    /// The part of Remit that writes it, set by the code that calls `emit`: the way the event came in follows from it.
    pub writer: Writer,
    /// Every header the writer set but the credentials.
    pub header: BTreeMap<String, String>,
    pub query: String,
    pub body: Vec<u8>,
    /// The writer's own id for the event, when it gives one: its retry carries the same.
    pub sender_id: String,
    /// Who wrote it and, for an agent's, its chain: a chain does not hear what it wrote itself.
    pub cause: Cause,
}

/// The event as `emit` stored it, or as it was stored already when the write is a retry.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Emitted {
    pub reading: Reading,
    pub event: InboundEvent,
}

impl Emitted {
    /// The address of the event, `event:<signal>#<rev>`.
    pub fn id(&self) -> String {
        event_ref(&self.reading)
    }
}

/// The way an event its writer of kind k writes comes in; none for a writer that writes no event.
fn via_of(k: &WriterKind) -> Option<EventVia> {
    match k {
        WriterKind::Door => Some(EventVia::Door),
        WriterKind::Cli => Some(EventVia::Cli),
        WriterKind::Sensor => Some(EventVia::Poll),
        WriterKind::Agent => Some(EventVia::Agent),
        WriterKind::Plugin => Some(EventVia::Plugin),
        _ => None,
    }
}

/// The dedupes under which a retry of e at now is the event already stored, the one a new event is stored under first. A
/// writer's own id is one, for good. With none, the body is, in the window of the time now falls in and in the one before
/// it, so a retry is told wherever it falls in the window's edge, and the same body a window or two later is a new event.
fn dedupes_of(signal: &str, e: &Emission, now: Time) -> Vec<String> {
    if !e.sender_id.is_empty() {
        return vec![format!("{signal}:{}", e.sender_id)];
    }
    let sum: String = Sha256::digest(&e.body).iter().map(|b| format!("{b:02x}")).collect();
    let window = (now.0.unix_timestamp_nanos() / 1_000_000).div_euclid(EVENT_RETRY_WINDOW_MS);
    vec![
        format!("{signal}:{sum}@{window}"),
        format!("{signal}:{sum}@{}", window.saturating_sub(1)),
    ]
}

/// The first limit bytes of b as text, cut at a character, and whether b was longer.
fn start_of(b: &[u8], limit: usize) -> (String, bool) {
    let Some(mut head) = b.get(..limit) else {
        return (String::from_utf8_lossy(b).into_owned(), false);
    };
    if head.len() == b.len() {
        return (String::from_utf8_lossy(b).into_owned(), false);
    }
    while let Err(e) = std::str::from_utf8(head) {
        head = head.get(..e.valid_up_to()).unwrap_or_default();
    }
    (String::from_utf8_lossy(head).into_owned(), true)
}

impl Pipeline {
    /// Stores an event of an event signal, once: a retry of one stored already (the same sender id, or else the same
    /// body) answers the stored one. It refuses a signal no one made, a paused one, and an event past what Remit can
    /// hold. The scheduler then lets the subscriptions to the signal act on it.
    pub async fn emit(&self, e: Emission) -> Result<Emitted> {
        let name = e.signal.trim().to_lowercase();
        if !is_event_signal_name(&name) {
            return Err(Error::NoEventSignal);
        }
        let Some(via) = via_of(&e.writer.kind) else {
            return Err(Error::refused(format!(
                "an event comes in by the door, the CLI, a sensor, an agent or a plugin, not by a {:?} writer",
                e.writer.kind.as_str()
            )));
        };
        let (body, body_cut) = start_of(&e.body, EVENT_BODY_MAX);
        let ev = InboundEvent {
            via,
            header: e.header.clone(),
            query: e.query.clone(),
            body,
            body_cut,
        };
        let value = ForeignJson(serde_json::to_value(EventValue { event: ev.clone() })?);
        let door = self.door();
        let w = Write {
            key: event_key(&name),
            value: Some(value),
            dedupes: dedupes_of(&name, &e, door.now),
            cause: e.cause,
            writer: e.writer,
            ..Write::default()
        };
        let named = name.clone();
        let ingested = self
            .store
            .update(move |tx| {
                match tx.event_signal_named(&named) {
                    Err(store::Error::NotFound) => return Err(Error::NoEventSignal),
                    found => found?,
                };
                door.ingest(tx, w)
            })
            .await?;
        if ingested.duplicate {
            let event = event_value_of(&ingested.reading).map(|v| v.event).unwrap_or_default();
            return Ok(Emitted {
                reading: ingested.reading,
                event,
            });
        }
        self.notify
            .event_read(&event_record_of(&ingested.reading, None, Vec::new()));
        self.kick();
        Ok(Emitted {
            reading: ingested.reading,
            event: ev,
        })
    }
}
