//! Event signals
//!
//! The key of an event signal's door is stored sealed in a column of its own, never in the record: nothing that reads
//! or pushes the record can carry it, and the only readers of the key are event_signal_named (the door checking a post)
//! and the owner asking to see it.

use model::EventSignal;
use rusqlite::params;
use secrets::Sealed;

use crate::store::{Error, Reader, Result, Tx, doc, unique_failed};

impl Tx<'_> {
    /// Stores es with its sealed key, at revision 1. `Error::EventSignalName` when another has its name: the name is
    /// its key and its door's address.
    pub fn create_event_signal(&self, mut es: EventSignal, sealed: &Sealed) -> Result<EventSignal> {
        es.rev = 1;
        let stored = self.exec(
            "INSERT INTO event_signals(id, name, sealed, key_id, doc) VALUES(?,?,?,?,?)",
            params![es.id, es.name, sealed.r#box, sealed.key_id, doc(&es)?],
        );
        match stored {
            Err(Error::Sqlite(e)) if unique_failed(&e, "event_signals.name") => return Err(Error::EventSignalName),
            Err(e) => return Err(e),
            Ok(1) => {}
            Ok(_) => return Err(Error::Refused("the event signal was not stored".into())),
        }
        Ok(es)
    }

    /// Writes es's record over the revision it was read at, leaves its key as it is, and returns it at the revision
    /// stored. `Error::Conflict` when the stored record is at another revision, or there is none.
    pub fn put_event_signal(&self, mut es: EventSignal) -> Result<EventSignal> {
        let read = es.rev;
        es.rev = read.saturating_add(1);
        es.rev = self.put_doc(
            "UPDATE event_signals SET doc=? WHERE id=? AND json_extract(doc, '$.rev')=? RETURNING json_extract(doc, '$.rev')",
            params![doc(&es)?, es.id, read],
        )?;
        Ok(es)
    }

    /// Writes es's record over the revision it was read at and a new sealed key together, and returns it at the revision
    /// stored. `Error::Conflict` as put_event_signal.
    pub fn rotate_event_signal(&self, mut es: EventSignal, sealed: &Sealed) -> Result<EventSignal> {
        let read = es.rev;
        es.rev = read.saturating_add(1);
        es.rev = self.put_doc(
            "UPDATE event_signals SET sealed=?, key_id=?, doc=? WHERE id=? AND json_extract(doc, '$.rev')=? RETURNING json_extract(doc, '$.rev')",
            params![sealed.r#box, sealed.key_id, doc(&es)?, es.id, read],
        )?;
        Ok(es)
    }

    pub fn delete_event_signal(&self, id: &str) -> Result<()> {
        self.exec("DELETE FROM event_signals WHERE id=?", [id])?;
        Ok(())
    }
}

impl Reader<'_> {
    pub fn event_signal(&self, id: &str) -> Result<EventSignal> {
        self.one("SELECT doc FROM event_signals WHERE id=?", [id])
    }

    /// The event signal with this name and its sealed key.
    pub fn event_signal_named(&self, name: &str) -> Result<(EventSignal, Sealed)> {
        let es = self.one("SELECT doc FROM event_signals WHERE name=?", [name])?;
        let sealed = self.sealed("SELECT sealed, key_id FROM event_signals WHERE name=?", name)?;
        Ok((es, sealed))
    }

    /// The sealed key of event signal id.
    pub fn sealed_event_signal_key(&self, id: &str) -> Result<Sealed> {
        self.sealed("SELECT sealed, key_id FROM event_signals WHERE id=?", id)
    }

    /// Every event signal, the newest first.
    pub fn event_signals(&self) -> Result<Vec<EventSignal>> {
        let mut all: Vec<EventSignal> = self.list("SELECT doc FROM event_signals", [])?;
        all.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| b.id.cmp(&a.id)));
        Ok(all)
    }
}
