//! Worker keys: the codes a mentor's devices hold to help through Remit. A key is stored sealed in a column of its own,
//! never in the record: nothing that reads or pushes a record can carry it.

use model::WorkerKey;
use rusqlite::params;
use secrets::Sealed;

use crate::store::{Reader, Result, Tx, doc};

impl Tx<'_> {
    /// Stores the new key k with its sealed value at revision 1.
    pub fn add_worker_key(&self, mut k: WorkerKey, sealed: &Sealed) -> Result<WorkerKey> {
        k.rev = 1;
        self.exec(
            "INSERT INTO worker_keys(id, mentor, sealed, key_id, doc) VALUES(?,?,?,?,?)",
            params![k.id, k.mentor, sealed.r#box, sealed.key_id, doc(&k)?],
        )?;
        Ok(k)
    }

    pub fn delete_worker_key(&self, id: &str) -> Result<()> {
        self.exec("DELETE FROM worker_keys WHERE id=?", [id])?;
        Ok(())
    }

    /// Deletes every key of the mentor and returns their ids.
    pub fn delete_worker_keys_of(&self, mentor: &str) -> Result<Vec<String>> {
        self.column("DELETE FROM worker_keys WHERE mentor=? RETURNING id", [mentor])
    }

    /// Stores k over the key of its id at the revision it was read at, its sealed key kept, and returns it at the
    /// revision stored. `Error::Conflict` when the stored key is at another revision, or there is none.
    pub fn update_worker_key(&self, mut k: WorkerKey) -> Result<WorkerKey> {
        let read = k.rev;
        k.rev = read.saturating_add(1);
        k.rev = self.put_doc(
            "UPDATE worker_keys SET doc=? WHERE id=? AND json_extract(doc, '$.rev')=? RETURNING json_extract(doc, '$.rev')",
            params![doc(&k)?, k.id, read],
        )?;
        Ok(k)
    }
}

impl Reader<'_> {
    pub fn worker_key(&self, id: &str) -> Result<WorkerKey> {
        self.one("SELECT doc FROM worker_keys WHERE id=?", [id])
    }

    /// The mentor's keys, the newest first.
    pub fn worker_keys(&self, mentor: &str) -> Result<Vec<WorkerKey>> {
        Ok(newest_first(
            self.list("SELECT doc FROM worker_keys WHERE mentor=?", [mentor])?,
        ))
    }

    /// Every mentor's keys, the newest first.
    pub fn all_worker_keys(&self) -> Result<Vec<WorkerKey>> {
        Ok(newest_first(self.list("SELECT doc FROM worker_keys", [])?))
    }

    /// The sealed key of worker key id.
    pub fn sealed_worker_key(&self, id: &str) -> Result<Sealed> {
        self.sealed("SELECT sealed, key_id FROM worker_keys WHERE id=?", id)
    }
}

fn newest_first(mut all: Vec<WorkerKey>) -> Vec<WorkerKey> {
    all.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| b.id.cmp(&a.id)));
    all
}
