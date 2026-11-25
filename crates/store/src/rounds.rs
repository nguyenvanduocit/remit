//! Rounds of competing attempts (ADR-0115, migration 125): one row each, the key result it is of columns so a key
//! result's rounds read from their index, and its state a column generated from its document, so the rounds still to
//! judge read from theirs.

use model::{Round, Serves};
use rusqlite::params;

use crate::store::{Reader, Result, Tx, doc, ms, over_read_rev};

impl Tx<'_> {
    /// Writes r over the revision it was read at, r.rev (0 for a new round), and returns it at the revision stored.
    /// `Error::Conflict` when the stored round is at another revision, or a new one's id is taken.
    pub fn put_round(&self, mut r: Round) -> Result<Round> {
        r.rev = r.rev.saturating_add(1);
        r.rev = self.put_doc(
            concat!(
                "INSERT INTO rounds(id, objective, key_result, number, created_at, doc) VALUES(?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET doc=excluded.doc",
                over_read_rev!("rounds"),
                " RETURNING json_extract(doc, '$.rev')"
            ),
            params![
                r.id,
                r.serves.objective,
                r.serves.key_result,
                r.number,
                ms(r.created_at),
                doc(&r)?
            ],
        )?;
        Ok(r)
    }
}

impl Reader<'_> {
    pub fn round(&self, id: &str) -> Result<Round> {
        self.one("SELECT doc FROM rounds WHERE id=?", [id])
    }

    /// The rounds of key result s, the first first.
    pub fn key_result_rounds(&self, s: &Serves) -> Result<Vec<Round>> {
        self.list(
            "SELECT doc FROM rounds WHERE objective=? AND key_result=? ORDER BY number",
            params![s.objective, s.key_result],
        )
    }

    /// The rounds not yet picked in or ended (open or judging), the oldest first.
    pub fn unsettled_rounds(&self) -> Result<Vec<Round>> {
        self.list(
            "SELECT doc FROM rounds WHERE state IN ('open', 'judging') ORDER BY created_at, id",
            [],
        )
    }
}
