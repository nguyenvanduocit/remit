//! Ingest is the one door of the signal log (docs/adr/0064): every reading, whoever writes it, enters through
//! `Door::ingest`, which decides for all of them whether the writer may write the key, whether the signal or the writing
//! sensor is paused, the reading's time and whether it is a retry of one stored already. It runs in the caller's
//! transaction, so a fact or a chain's record is stored with the change that caused it or not at all; the caller kicks the
//! scheduler after the commit, and the Tick judges what came in.

use std::sync::Arc;

use model::{Cause, ForeignJson, Reading, SignalKey, Time, Writer, WriterKind};
use store::Tx;

use crate::chain_core::Core;
use crate::signal_sources::event_signal_of;
use crate::{Error, Result};
use std::sync::PoisonError;

/// One reading as its writer hands it to ingest.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Write {
    pub key: SignalKey,
    /// What the reading is about within its key; empty: the key itself.
    pub subject: String,
    pub value: Option<ForeignJson>,
    /// The writer's ids for this occurrence, the first stored with the reading: a retry carries one of them (an event's
    /// body hash gives the window it falls in and the one before).
    pub dedupes: Vec<String>,
    pub cause: Cause,
    /// Set by the code that writes, never read from a payload.
    pub writer: Writer,
}

/// What ingest made of a write: the reading stored, and whether it was stored already (a retry), which is an outcome and
/// not an error.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ingested {
    pub reading: Reading,
    pub duplicate: bool,
}

/// The event signals the plugins bundled with Remit ship, each under its plugin's id: a plugin writes only these.
#[derive(Debug, Clone, Default)]
pub struct ShippedSignals(pub Vec<(String, plugin_sdk::Signal)>);

impl ShippedSignals {
    /// Whether bundled plugin id ships the event signal name.
    pub fn ships(&self, id: &str, name: &str) -> bool {
        self.0.iter().any(|(plugin, s)| plugin == id && s.name == name)
    }
}

/// What ingest weighs besides the store: the time it gives a reading, and what the bundled plugins ship. A copy goes
/// into each transaction that ingests (`Core::door`).
#[derive(Debug, Clone)]
pub struct Door {
    pub now: Time,
    shipped: Arc<ShippedSignals>,
}

impl Door {
    /// Stores w in tx as the next reading of its key, at the door's now: refused when its writer may not write the key or
    /// the signal or the sensor is paused, answered with the stored reading when one of its dedupes is stored already for
    /// the key.
    pub fn ingest(&self, tx: &Tx<'_>, w: Write) -> Result<Ingested> {
        may_write(&w.writer, &w.key, &self.shipped)?;
        writable(tx, &w)?;
        for dedupe in &w.dedupes {
            match tx.reading_deduped(&w.key, dedupe) {
                Ok(reading) => {
                    return Ok(Ingested {
                        reading,
                        duplicate: true,
                    });
                }
                Err(store::Error::NotFound) => {}
                Err(err) => return Err(err.into()),
            }
        }
        let r = Reading {
            dedupe: w.dedupes.first().cloned(),
            key: w.key,
            subject: w.subject,
            at: self.now,
            cause: w.cause,
            value: w.value,
            writer: w.writer,
            ..Reading::default()
        };
        let key = r.key.clone();
        let reading = tx
            .add_reading(r)
            .map_err(|e| Error::refused(format!("store the reading of {key}: {e}")))?;
        Ok(Ingested {
            reading,
            duplicate: false,
        })
    }
}

impl Core {
    /// The door a transaction begun now ingests through.
    pub fn door(&self) -> Door {
        Door {
            now: self.now(),
            shipped: self.shipped_signals(),
        }
    }
}

/// Refuses a write whose writer is not named, or may not write key: a plugin writes only the event signals it ships.
fn may_write(by: &Writer, key: &SignalKey, shipped: &ShippedSignals) -> Result<()> {
    if by.kind != WriterKind::Plugin {
        return Ok(());
    }
    match event_signal_of(key) {
        Some(name) if shipped.ships(&by.id, name) => Ok(()),
        _ => Err(Error::refused(format!(
            "plugin {} writes only the event signals it ships, and {key} is not one",
            by.id
        ))),
    }
}

/// Refuses w while the event signal its key is a reading of is paused, or the sensor that writes it is.
fn writable(tx: &Tx<'_>, w: &Write) -> Result<()> {
    if let Some(name) = event_signal_of(&w.key) {
        match tx.event_signal_named(name) {
            Ok((es, _)) if es.paused => return Err(Error::SignalPaused),
            Ok(_) | Err(store::Error::NotFound) => {}
            Err(err) => return Err(err.into()),
        }
    }
    if w.writer.kind == WriterKind::Sensor {
        match tx.sensor(&w.writer.id) {
            Ok(sn) if sn.paused => return Err(Error::SignalPaused),
            Ok(_) | Err(store::Error::NotFound) => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(())
}

impl Core {
    /// What the bundled plugins ship, as last loaded (`load_shipped`).
    pub fn shipped_signals(&self) -> Arc<ShippedSignals> {
        Arc::clone(&self.shipped.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

/// Who a plugin's processor is to the gate: the caller of the lend that gives it its connection's variables, in no chain.
pub const PROCESSOR_AGENT: &str = "processor";

/// Who Remit's poller is to the gate: the caller of every poll it makes, in no chain.
pub const SENSOR_AGENT: &str = "sensor";
