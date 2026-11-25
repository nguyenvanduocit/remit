//! Duels (migration 89): two lineages at the same work and the judge's pick, one row each, the kind of work a column so
//! a lineage's rating reads the duels of one kind from its index.

use model::Duel;
use rusqlite::params;

use crate::store::{Reader, Result, Tx, doc, ms, over_read_rev};

impl Tx<'_> {
    /// Writes d over the revision it was read at, d.rev (0 for a new duel), and returns it at the revision stored.
    /// `Error::Conflict` when the stored duel is at another revision, or a new one's id is taken.
    pub fn put_duel(&self, mut d: Duel) -> Result<Duel> {
        d.rev = d.rev.saturating_add(1);
        d.rev = self.put_doc(
            concat!(
                "INSERT INTO duels(id, chain, work, created_at, doc) VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET doc=excluded.doc",
                over_read_rev!("duels"),
                " RETURNING json_extract(doc, '$.rev')"
            ),
            params![d.id, d.chain, d.work, ms(d.created_at), doc(&d)?],
        )?;
        Ok(d)
    }
}

impl Reader<'_> {
    /// The duels of chain chain_id, the oldest first.
    pub fn chain_duels(&self, chain_id: &str) -> Result<Vec<Duel>> {
        self.list(
            "SELECT doc FROM duels WHERE chain=? ORDER BY created_at, id",
            [chain_id],
        )
    }

    /// Every duel, the oldest first: what the ratings of every lineage are computed from.
    pub fn duels(&self) -> Result<Vec<Duel>> {
        self.list("SELECT doc FROM duels ORDER BY created_at, id", [])
    }
}
