//! Retrospectives (ADR-0082): what the coach wrote after each finish of a chain. Chains finish without bound, so they
//! are rows of their own beside the chain, carried in its view, and they go with it (`delete_home`).

use model::Retrospective;
use rusqlite::params;

use crate::store::{Reader, Result, Tx, doc, ms, over_read_rev};

impl Tx<'_> {
    /// Writes r over the revision it was read at, r.rev (0 for a new one), and returns it at the revision stored.
    /// `Error::Conflict` when the stored one is at another revision, or a new one's id is taken.
    pub fn put_retrospective(&self, mut r: Retrospective) -> Result<Retrospective> {
        r.rev = r.rev.saturating_add(1);
        r.rev = self.put_doc(
            concat!(
                "INSERT INTO retrospectives(id, chain_id, created_at, doc) VALUES(?,?,?,?) ON CONFLICT(id) DO UPDATE SET doc=excluded.doc",
                over_read_rev!("retrospectives"),
                " RETURNING json_extract(doc, '$.rev')"
            ),
            params![r.id, r.chain_id, ms(r.created_at), doc(&r)?],
        )?;
        Ok(r)
    }
}

impl Reader<'_> {
    /// A chain's retrospectives, oldest first.
    pub fn retrospectives_of(&self, chain_id: &str) -> Result<Vec<Retrospective>> {
        self.list(
            "SELECT doc FROM retrospectives WHERE chain_id=? ORDER BY created_at, rowid",
            [chain_id],
        )
    }
}
