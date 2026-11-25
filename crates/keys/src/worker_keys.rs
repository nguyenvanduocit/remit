//! A worker key is a code a mentor works from: the key a browser of theirs signs in to the web app with (ADR-0060,
//! ADR-0062). The owner has the Assistant make one for a mentor (worker_key_tools.rs), as many as they hold devices, reads
//! its value on the mentor's row and passes it on; the mentor pastes it into their app. It is made, sealed and matched as
//! a client key is (entrance_keys.rs, `authorize_client_key`): 256 random bits behind "wk_", sealed for the key's record,
//! compared in constant time over every key. The mentor's removal and the Assistant on the owner's words delete it, and the
//! channel closes its sockets (`Notifier::worker_key_deleted`).

use model::{Actor, MentorStatus, Time, WorkerKey};
use subtle::ConstantTimeEq;

use crate::Keys;
use crate::client_keys::LAST_USED_EVERY;
use chains::chain_core::new_id;
use chains::{Error, Result};

/// What every worker key starts with, so a secret scanner knows one.
const WORKER_KEY_PREFIX: &str = "wk_";

/// The longest note a code takes: it names a device in the owner's words.
const WORKER_KEY_NOTE_MAX: usize = 100;

fn worker_key_aad(id: &str) -> String {
    format!("worker/{id}")
}

impl Keys {
    /// Which key the presented value is: one that exists and whose value it is. Every way of failing gives the same
    /// answer, so a holder learns nothing about which keys exist. A key that opens is noted as used from addr.
    pub async fn authorize_worker_key(&self, presented: &str, addr: &str) -> Result<Option<WorkerKey>> {
        if !presented.starts_with(WORKER_KEY_PREFIX) {
            return Ok(None);
        }
        let keys = self
            .store
            .read(|r| {
                r.all_worker_keys()?
                    .into_iter()
                    .map(|k| r.sealed_worker_key(&k.id).map(|s| (k, s)))
                    .collect::<store::Result<Vec<_>>>()
            })
            .await?;
        let mut matched: Option<WorkerKey> = None;
        for (k, sealed) in keys {
            let value = match self.opened_key(&worker_key_aad(&k.id), &sealed) {
                Ok(value) => value,
                Err(err) => {
                    tracing::error!(key = %k.id, mentor = %k.mentor, %err, "a worker key cannot be opened");
                    continue;
                }
            };
            // Every key is compared, so the time taken does not tell how many were tried before the right one.
            if bool::from(value.as_bytes().ct_eq(presented.as_bytes())) {
                matched = Some(k);
            }
        }
        match matched {
            Some(k) => Ok(Some(self.note_worker_key_used(k, addr, self.now()).await)),
            None => Ok(None),
        }
    }

    /// Stores that key k opened the channel from addr, at most once a minute, and returns it as stored. A failure to store
    /// it is logged: the holder is let in all the same.
    async fn note_worker_key_used(&self, mut k: WorkerKey, addr: &str, now: Time) -> WorkerKey {
        let recent = k
            .last_used_at
            .and_then(|last| last.0.checked_add(LAST_USED_EVERY))
            .is_some_and(|next| now.0 < next);
        if recent && k.last_addr == addr {
            return k;
        }
        k.last_used_at = Some(now);
        addr.clone_into(&mut k.last_addr);
        let used = k.clone();
        match self.store.update(move |tx| tx.update_worker_key(used)).await {
            Ok(saved) => {
                self.notify.mentors_moved(); // the code's line says when it was last used
                saved
            }
            Err(err) => {
                tracing::error!(key = %k.id, %err, "a worker key's use is not stored");
                k
            }
        }
    }

    /// Makes a new code for the active mentor, noted as the owner says what it is for, by who asked, and tells the
    /// channels. Its value is sealed apart from the record: only the owner is shown it (`worker_key`).
    pub async fn create_worker_key(&self, mentor: &str, note: &str, by: &Actor) -> Result<WorkerKey> {
        let note = note.trim().to_owned();
        if note.chars().count() > WORKER_KEY_NOTE_MAX {
            return Err(Error::refused(format!(
                "a code's note is at most {WORKER_KEY_NOTE_MAX} characters"
            )));
        }
        let id = mentor.to_owned();
        let m = match self.store.read(move |r| r.mentor(&id)).await {
            Err(store::Error::NotFound) => {
                return Err(Error::refused(format!(
                    "no mentor {mentor:?}: list_mentors shows the ids"
                )));
            }
            other => other?,
        };
        if m.status != MentorStatus::Active {
            return Err(Error::refused(format!(
                "{} was removed from the mentors: a code is made for a mentor in the pool",
                m.label
            )));
        }
        let k = WorkerKey {
            id: new_id("wkey_"),
            mentor: m.id.clone(),
            note,
            created_at: self.now(),
            ..WorkerKey::default()
        };
        let sealed = self.seal_new_key(WORKER_KEY_PREFIX, &worker_key_aad(&k.id)).await?;
        let k = self.store.update(move |tx| tx.add_worker_key(k, &sealed)).await?;
        tracing::info!(key = %k.id, mentor = %m.id, by = %by.kind, who = %by.who, "worker key made");
        self.notify.mentors_moved();
        Ok(k)
    }

    /// Code id's value, for the owner who asked to see it to pass it on: the one place a channel is given it. It is read,
    /// never pushed, and the server's log says who asked. An agent that asks is told to have the owner open the mentor's
    /// row.
    pub async fn worker_key(&self, id: &str, by: &Actor) -> Result<String> {
        chains::owner_acts::for_owner(chains::owner_acts::ACT_SEE_KEY, by)?;
        let key = id.to_owned();
        let sealed = match self.store.read(move |r| r.sealed_worker_key(&key)).await {
            Err(store::Error::NotFound) => {
                return Err(Error::refused(format!(
                    "no code {id:?}: it was revoked, or the mentor removed"
                )));
            }
            other => other?,
        };
        let value = self
            .opened_key(&worker_key_aad(id), &sealed)
            .map_err(|e| Error::refused(format!("the code {e}; or revoke it and have a new one made")))?;
        tracing::info!(key = %id, by = %by.kind, who = %by.who, "worker key shown");
        Ok(value)
    }

    /// Deletes key id and tells the channels, which close its sessions. A key already gone is no error.
    pub async fn revoke_worker_key(&self, id: &str) -> Result<()> {
        let key = id.to_owned();
        match self.store.update(move |tx| tx.delete_worker_key(&key)).await {
            Ok(()) | Err(store::Error::NotFound) => {}
            Err(err) => return Err(err.into()),
        }
        self.notify.worker_key_deleted(id);
        Ok(())
    }
}
