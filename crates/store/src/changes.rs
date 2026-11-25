//! A change is the record of one write of an agent, a skill, an MCP server or a playbook. It is part of a Tx with that
//! write, and is only ever inserted.

use model::{Change, ChangeKind};
use rusqlite::params;

use crate::store::{Reader, Result, Tx, doc, ms};

impl Tx<'_> {
    pub fn put_change(&self, c: &Change) -> Result<()> {
        self.exec(
            "INSERT INTO changes(id, kind, name, at, doc) VALUES(?,?,?,?,?)",
            params![c.id, c.kind.as_str(), c.name, ms(c.at), doc(c)?],
        )?;
        Ok(())
    }
}

impl Reader<'_> {
    pub fn change(&self, id: &str) -> Result<Change> {
        self.one("SELECT doc FROM changes WHERE id=?", [id])
    }

    /// The writes of one record, newest first, at most limit.
    pub fn changes(&self, kind: &ChangeKind, name: &str, limit: i64) -> Result<Vec<Change>> {
        self.list(
            "SELECT doc FROM changes WHERE kind=? AND name=? ORDER BY at DESC, rowid DESC LIMIT ?",
            params![kind.as_str(), name, limit],
        )
    }
}
