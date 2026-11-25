//! Seeded faults (ADR-0102): the fault a trial chain's planter put in its work on purpose. One per chain at most, kept
//! beside the chain and gone with it (`delete_home`); no agent's tool reads them, only the outcome report.

use model::SeededFault;
use rusqlite::params;

use crate::store::{Reader, Result, Tx, doc, ms};

impl Tx<'_> {
    /// Stores f as its chain's fault; false when the chain has one already, which stays as it is.
    pub fn put_seeded_fault(&self, f: &SeededFault) -> Result<bool> {
        let n = self.exec(
            "INSERT INTO seeded_faults(id, chain_id, planted_at, doc) VALUES(?,?,?,?) ON CONFLICT(chain_id) DO NOTHING",
            params![f.id, f.chain_id, ms(f.planted_at), doc(f)?],
        )?;
        Ok(n == 1)
    }
}

impl Reader<'_> {
    /// Every chain's seeded fault, the oldest planted first.
    pub fn seeded_faults(&self) -> Result<Vec<SeededFault>> {
        self.list("SELECT doc FROM seeded_faults ORDER BY planted_at, rowid", [])
    }
}
