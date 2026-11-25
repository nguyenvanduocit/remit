//! An event is a reading of an event signal, and what became of it is the firings of the subscriptions that held for it
//! (ADR-0036). An EventRecord is the two together, as the Events page and the Assistant read them; nothing else says what
//! became of an event.

use model::{EVENT_KEY_PREFIX, EventRef, InboundEvent, Reading, event_key, parse_event_ref};
use wire::app::EventRecord;

use crate::Pipeline;
use crate::event_signal::event_value_of;
use crate::signalboard::{firing_lines, namer_of};
use chains::signal_sources::signal_key_of;
use chains::{Error, Result};
use wire::app::FiringLine;

/// The name of the event signal reading r is of.
fn signal_name(r: &Reading) -> String {
    signal_key_of(&r.key).map(|k| k.name.to_owned()).unwrap_or_default()
}

impl Pipeline {
    /// The record of reading r as the store holds it now, its firings named by the subscriptions to its key; whole, with
    /// what it carries.
    async fn event_record(&self, r: Reading, whole: bool) -> Result<EventRecord> {
        let (key, seq) = (r.key.clone(), r.seq);
        let (subs, fired) = self
            .store
            .read(move |s| Ok::<_, store::Error>((s.subscriptions_on_key(&key)?, s.firings_on(&key, seq)?)))
            .await?;
        let firings = firing_lines(&fired, &namer_of(&[], &subs, &[]));
        let event = whole.then(|| event_value_of(&r).map(|v| v.event)).flatten();
        Ok(event_record_of(&r, event, firings))
    }

    /// The event the address id names, whole; refused for an address that is none, a signal that has no such reading, or
    /// one forgotten.
    pub async fn event_get(&self, id: &str) -> Result<EventRecord> {
        let Some(at) = parse_event_ref(id) else {
            return Err(Error::refused(format!(
                "{id:?} is no event's address: event:<signal>#<rev>"
            )));
        };
        let key = event_key(&at.signal);
        let r = self.store.read(move |s| s.reading(&key, at.rev)).await?;
        self.event_record(r, true).await
    }

    /// The newest n events, the newest first, without what arrived.
    pub async fn recent_events(&self, n: i64) -> Result<Vec<EventRecord>> {
        let readings = self.store.read(move |s| s.newest_event_readings(n)).await?;
        let mut out = Vec::with_capacity(readings.len());
        for r in readings {
            out.push(self.event_record(r, false).await?);
        }
        Ok(out)
    }

    /// Tells the channels what became of the events a firing covered, which are the readings of an event key.
    pub(crate) async fn events_fired(&self, readings: &[Reading]) {
        for r in readings.iter().filter(|r| r.key.0.starts_with(EVENT_KEY_PREFIX)) {
            match self.event_record(r.clone(), false).await {
                Ok(rec) => self.notify.event_read(&rec),
                Err(err) => tracing::warn!(key = %r.key.0, rev = r.rev, %err, "an event's record not pushed"),
            }
        }
    }
}

/// The address of event reading r.
pub fn event_ref(r: &Reading) -> String {
    EventRef {
        signal: signal_name(r),
        rev: r.rev,
    }
    .to_string()
}

/// The record of reading r with its firings; event is what it carries, or none.
pub fn event_record_of(r: &Reading, event: Option<InboundEvent>, firings: Vec<FiringLine>) -> EventRecord {
    EventRecord {
        id: event_ref(r),
        signal: signal_name(r),
        rev: r.rev,
        at: r.at,
        via: event_value_of(r).map(|v| v.event.via).unwrap_or_default(),
        event,
        firings,
    }
}
