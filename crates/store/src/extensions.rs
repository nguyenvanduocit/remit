//! Extensions
//!
//! The sources the owner applied as programs. One manifest per id; applying one again replaces the manifest before it.
//! Who proposed it and who allowed it is the Change recorded with it, as for every record the owner has an agent write.

use model::{DisabledExtension, StoredExtension};
use rusqlite::params;

use crate::store::{Reader, Result, Tx, doc, from_ms, ms};

impl Tx<'_> {
    /// Keeps e as the manifest of its id.
    pub fn put_extension(&self, e: &StoredExtension) -> Result<()> {
        self.exec(
            "INSERT INTO extensions(id, applied_at, doc) VALUES(?,?,?)
ON CONFLICT(id) DO UPDATE SET applied_at=excluded.applied_at, doc=excluded.doc",
            params![e.id, ms(e.applied_at), e.doc],
        )?;
        Ok(())
    }

    /// Removes the manifest of id; one that is not there is already removed.
    pub fn delete_extension(&self, id: &str) -> Result<()> {
        self.exec("DELETE FROM extensions WHERE id=?", [id])?;
        Ok(())
    }

    /// Keeps that the owner disabled d.id; disabling it again replaces who and when.
    pub fn put_disabled_extension(&self, d: &DisabledExtension) -> Result<()> {
        self.exec(
            "INSERT INTO disabled_extensions(id, doc) VALUES(?,?) ON CONFLICT(id) DO UPDATE SET doc=excluded.doc",
            params![d.id, doc(d)?],
        )?;
        Ok(())
    }

    /// Forgets that id was disabled; one that is not disabled is already enabled.
    pub fn delete_disabled_extension(&self, id: &str) -> Result<()> {
        self.exec("DELETE FROM disabled_extensions WHERE id=?", [id])?;
        Ok(())
    }
}

impl Reader<'_> {
    /// The extensions applied, by id.
    pub fn extensions(&self) -> Result<Vec<StoredExtension>> {
        let mut stmt = self
            .c
            .prepare_cached("SELECT id, applied_at, doc FROM extensions ORDER BY id")?;
        let rows = stmt.query_map([], |r| {
            Ok(StoredExtension {
                id: r.get(0)?,
                applied_at: from_ms(r.get(1)?),
                doc: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The extensions the owner disabled, by id.
    pub fn disabled_extensions(&self) -> Result<Vec<DisabledExtension>> {
        self.list("SELECT doc FROM disabled_extensions ORDER BY id", [])
    }
}
