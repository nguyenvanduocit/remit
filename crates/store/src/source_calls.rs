//! Source calls: the account of what agents asked of the outside world (ADR-0018, invariant 3). A call is written once
//! and never changed, and is never pruned with its chain: it is the owner's record of what an agent did to a system
//! outside Remit. It lives in the database, which no agent session can read.

use model::SourceCall;
use rusqlite::params;

use crate::store::{Reader, Result, Tx, doc, ms};

impl Tx<'_> {
    pub fn add_source_call(&self, c: &SourceCall) -> Result<()> {
        self.exec(
            "INSERT INTO source_calls(chain_id, at, doc) VALUES(?,?,?)",
            params![c.chain, ms(c.at), doc(c)?],
        )?;
        Ok(())
    }
}

impl Reader<'_> {
    /// The newest limit calls the chain made, oldest first.
    pub fn source_calls(&self, chain_id: &str, limit: i64) -> Result<Vec<SourceCall>> {
        self.list(
            "SELECT doc FROM (SELECT id, doc FROM source_calls WHERE chain_id=? ORDER BY id DESC LIMIT ?) ORDER BY id",
            params![chain_id, limit],
        )
    }
}
