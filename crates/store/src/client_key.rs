//! Client keys
//!
//! A key is stored sealed in a column of its own, never in the record (as an event signal's is): nothing that reads or
//! pushes the record can carry it. Its readers are the entrance that checks a key presented to it and the owner asking
//! to see it.

use model::{ClientKey, Time};
use rusqlite::params;
use secrets::Sealed;

use crate::store::{Error, Reader, Result, Tx, doc, found, ms, unique_failed};

impl Tx<'_> {
    /// Stores k with its sealed key, at revision 1. `Error::ClientKeyName` when another key has its name.
    pub fn create_client_key(&self, mut k: ClientKey, sealed: &Sealed) -> Result<ClientKey> {
        k.rev = 1;
        let stored = self.exec(
            "INSERT INTO client_keys(id, name, sealed, key_id, doc) VALUES(?,?,?,?,?)",
            params![k.id, k.name, sealed.r#box, sealed.key_id, doc(&k)?],
        );
        match stored {
            Err(Error::Sqlite(e)) if unique_failed(&e, "client_keys.name") => return Err(Error::ClientKeyName),
            Err(e) => return Err(e),
            Ok(1) => {}
            Ok(_) => return Err(Error::Refused("the client key was not stored".into())),
        }
        Ok(k)
    }

    /// Writes k's record over the revision it was read at, leaves its key as it is, and returns it at the revision
    /// stored. `Error::Conflict` when the stored record is at another revision, or there is none.
    pub fn put_client_key(&self, mut k: ClientKey) -> Result<ClientKey> {
        let read = k.rev;
        k.rev = read.saturating_add(1);
        k.rev = self.put_doc(
            "UPDATE client_keys SET doc=? WHERE id=? AND json_extract(doc, '$.rev')=? RETURNING json_extract(doc, '$.rev')",
            params![doc(&k)?, k.id, read],
        )?;
        Ok(k)
    }

    /// Writes k's record over the revision it was read at and a new sealed key together, and returns it at the revision
    /// stored. `Error::Conflict` as put_client_key.
    pub fn rotate_client_key(&self, mut k: ClientKey, sealed: &Sealed) -> Result<ClientKey> {
        let read = k.rev;
        k.rev = read.saturating_add(1);
        k.rev = self.put_doc(
            "UPDATE client_keys SET sealed=?, key_id=?, doc=? WHERE id=? AND json_extract(doc, '$.rev')=? RETURNING json_extract(doc, '$.rev')",
            params![sealed.r#box, sealed.key_id, doc(&k)?, k.id, read],
        )?;
        Ok(k)
    }

    pub fn delete_client_key(&self, id: &str) -> Result<()> {
        self.exec("DELETE FROM client_keys WHERE id=?", [id])?;
        Ok(())
    }
}

impl Reader<'_> {
    pub fn client_key(&self, id: &str) -> Result<ClientKey> {
        self.one("SELECT doc FROM client_keys WHERE id=?", [id])
    }

    /// The sealed key of client key id.
    pub fn sealed_client_key(&self, id: &str) -> Result<Sealed> {
        self.sealed("SELECT sealed, key_id FROM client_keys WHERE id=?", id)
    }

    /// The sealed key the row query reads by arg.
    pub(crate) fn sealed(&self, query: &str, arg: &str) -> Result<Sealed> {
        found(self.c.prepare_cached(query)?.query_row([arg], |r| {
            Ok(Sealed {
                r#box: r.get(0)?,
                key_id: r.get(1)?,
            })
        }))
    }

    /// Every client key, the newest first.
    pub fn client_keys(&self) -> Result<Vec<ClientKey>> {
        let mut all: Vec<ClientKey> = self.list("SELECT doc FROM client_keys", [])?;
        all.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| b.id.cmp(&a.id)));
        Ok(all)
    }

    /// How many actions the client key key_id asked for since the time: the requests it made, which are kept long
    /// enough to count any hour.
    pub fn client_acts_since(&self, key_id: &str, since: Time) -> Result<i64> {
        self.value(
            "SELECT count(*) FROM requests WHERE json_extract(by, '$.kind')='client' AND json_extract(by, '$.who')=? AND at>=?",
            params![key_id, ms(since)],
        )
    }

    /// How many chains the client key key_id started since the time.
    pub fn client_chains_since(&self, key_id: &str, since: Time) -> Result<i64> {
        self.value(
            "SELECT count(*) FROM chains WHERE json_extract(doc, '$.origin')='client' AND json_extract(doc, '$.by.who')=? AND created_at>=?",
            params![key_id, ms(since)],
        )
    }
}
