//! Packing
//!
//! An event changes while its turn runs (streamed text appended to it, a tool call updated) and is history after: read
//! when the owner opens its activity, and never changed again. Events are most of the database and their text and data
//! most of them, so once an event has not changed for a while its text and data are packed (raw DEFLATE) where they
//! are, never deleted: the design keeps every session's record. A packed value is stored as a BLOB and a plain one as
//! TEXT, so the storage class says which: a read unpacks what it finds (`stored`), and every write, which writes plain
//! text, leaves the value plain, with no flag to keep in step with it. Packing changes how an event is stored, not what
//! it is: its revision stays and no channel is told.

use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use model::Time;
use rusqlite::Row;
use rusqlite::types::{Type, Value, ValueRef};

use crate::store::{Reader, Result, Store, Tx, ms};

/// The shortest value worth packing, in bytes. On made-up agent text, the values from 1 KB up (one in eight) hold about
/// three quarters of what packing every value from 256 bytes would save, so a read of an old chain's events unpacks few
/// values for most of the gain.
const PACK_FROM: usize = 1024;

/// How many events one write transaction packs: few, so the one writer is never held for long.
const PACK_BATCH: i64 = 200;

/// An event's text or data at column i as it is stored, plain (TEXT) or packed (a BLOB), unpacked; None for NULL.
pub(crate) fn stored(r: &Row<'_>, i: usize) -> rusqlite::Result<Option<String>> {
    let fail =
        |ty: Type, e: Box<dyn std::error::Error + Send + Sync>| rusqlite::Error::FromSqlConversionFailure(i, ty, e);
    match r.get_ref(i)? {
        ValueRef::Null => Ok(None),
        ValueRef::Text(t) => String::from_utf8(t.to_vec())
            .map(Some)
            .map_err(|e| fail(Type::Text, Box::new(e))),
        ValueRef::Blob(b) => unpack(b)
            .map(Some)
            .map_err(|e| fail(Type::Blob, format!("unpack an event: {e}").into())),
        ValueRef::Integer(_) => Err(rusqlite::Error::InvalidColumnType(
            i,
            "an event's value".into(),
            Type::Integer,
        )),
        ValueRef::Real(_) => Err(rusqlite::Error::InvalidColumnType(
            i,
            "an event's value".into(),
            Type::Real,
        )),
    }
}

/// A packed value's text.
pub(crate) fn unpack(packed: &[u8]) -> std::io::Result<String> {
    let mut out = String::new();
    DeflateDecoder::new(packed).read_to_string(&mut out)?;
    Ok(out)
}

/// An event `pack_events` packs: its text and data as stored, at the revision they were read.
pub(crate) struct ToPack {
    id: i64,
    rev: i64,
    text: Value,
    data: Value,
}

impl Store {
    /// Packs the events last changed before before that hold a value worth packing, a batch at a time, each batch in a
    /// write transaction of its own, and says how many it packed. An event changed since it was read is left as it is.
    /// Dropping the future stops it between two batches; what it packed stays packed.
    pub async fn pack_events(&self, before: Time) -> Result<i64> {
        let (mut packed, mut after) = (0_i64, 0_i64);
        loop {
            // Read and deflated on a blocking thread, outside the write transaction, so the writer is held only to store.
            let batch = self
                .read(move |r| {
                    r.events_to_pack(before, after)?
                        .into_iter()
                        .map(deflate_event)
                        .collect::<Result<Vec<_>>>()
                })
                .await?;
            let Some(last) = batch.last() else { return Ok(packed) };
            after = last.id;
            let n = self.update(move |tx| tx.pack_all(&batch)).await?;
            packed = packed.saturating_add(n);
        }
    }
}

impl Reader<'_> {
    /// The next PACK_BATCH events after event id after that were last changed before before and hold a value worth
    /// packing, read in id order.
    fn events_to_pack(&self, before: Time, after: i64) -> Result<Vec<ToPack>> {
        let from = i64::try_from(PACK_FROM).unwrap_or(i64::MAX);
        let mut stmt = self.c.prepare_cached(
            "SELECT id, rev, text, data FROM events WHERE id > ? AND updated_at < ? AND ((typeof(text)='text' AND length(text) >= ?) OR (typeof(data)='text' AND length(data) >= ?)) ORDER BY id LIMIT ?",
        )?;
        let rows = stmt.query_map(rusqlite::params![after, ms(before), from, from, PACK_BATCH], |r| {
            Ok(ToPack {
                id: r.get(0)?,
                rev: r.get(1)?,
                text: r.get(2)?,
                data: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

impl Tx<'_> {
    /// Stores batch packed and says how many events it changed: one written since it was read (another revision) keeps
    /// what that write left.
    fn pack_all(&self, batch: &[ToPack]) -> Result<i64> {
        let mut n = 0_i64;
        for e in batch {
            let changed = self.exec(
                "UPDATE events SET text=?, data=? WHERE id=? AND rev=?",
                rusqlite::params![e.text, e.data, e.id, e.rev],
            )?;
            n = n.saturating_add(i64::try_from(changed).unwrap_or(0));
        }
        Ok(n)
    }
}

fn deflate_event(e: ToPack) -> Result<ToPack> {
    Ok(ToPack {
        text: deflate(e.text)?,
        data: deflate(e.data)?,
        ..e
    })
}

/// v packed when it is plain text worth packing and packs smaller, else v as it is.
fn deflate(v: Value) -> Result<Value> {
    let Value::Text(plain) = v else { return Ok(v) };
    if plain.len() < PACK_FROM {
        return Ok(Value::Text(plain));
    }
    let mut w = DeflateEncoder::new(Vec::new(), Compression::default());
    w.write_all(plain.as_bytes())?;
    let packed = w.finish()?;
    if packed.len() >= plain.len() {
        return Ok(Value::Text(plain));
    }
    Ok(Value::Blob(packed))
}
