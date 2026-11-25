//! Seen marks (ADR-0079): what the owner last viewed of each subject, a home or the Assistant's feed. A mark is not a
//! field on the record it annotates, so marking moves no chain's revision and reorders no column. It goes with its home
//! (`delete_home`).

use model::Seen;
use rusqlite::{Row, params};

use crate::store::{Reader, Result, Tx, from_ms, ms};

fn scan_seen(r: &Row<'_>) -> rusqlite::Result<Seen> {
    Ok(Seen {
        subject: r.get(0)?,
        event_id: r.get(1)?,
        at: from_ms(r.get(2)?),
    })
}

impl Tx<'_> {
    /// Moves the mark of s.subject forward to s, and says whether it moved. A mark at a newer event stays, so a late or
    /// replayed call changes nothing; at the same event only a later time moves it (the feed's marks are all at event 0),
    /// and its time never goes back.
    pub fn put_seen(&self, s: &Seen) -> Result<bool> {
        let n = self.exec(
            "INSERT INTO seen(subject, event_id, at) VALUES(?,?,?) ON CONFLICT(subject) DO UPDATE
             SET event_id=excluded.event_id, at=max(seen.at, excluded.at)
             WHERE excluded.event_id > seen.event_id OR (excluded.event_id = seen.event_id AND excluded.at > seen.at)",
            params![s.subject, s.event_id, ms(s.at)],
        )?;
        Ok(n > 0)
    }
}

impl Reader<'_> {
    /// The owner's mark on subject; none before he viewed it.
    pub fn seen(&self, subject: &str) -> Result<Option<Seen>> {
        Ok(self
            .seen_rows("SELECT subject, event_id, at FROM seen WHERE subject=?", [subject])?
            .pop())
    }

    /// Every mark, by subject.
    pub fn seen_all(&self) -> Result<Vec<Seen>> {
        self.seen_rows("SELECT subject, event_id, at FROM seen ORDER BY subject", [])
    }

    fn seen_rows(&self, query: &str, params: impl rusqlite::Params) -> Result<Vec<Seen>> {
        let mut stmt = self.c.prepare_cached(query)?;
        let rows = stmt.query_map(params, scan_seen)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}
