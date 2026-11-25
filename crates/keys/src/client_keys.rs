//! A client key (`model::ClientKey`) is a way in the owner opened for one outside agent, the third beside the web app and
//! the event signals' doors: a key only that agent presents to the endpoint /mcp/client, and the limits on what it may
//! start. He never fills in a form for one: he tells the Assistant, whose tools (client_key_tools.rs) are the actions here.
//! The key is made by Remit, sealed under the key file beside the database and kept apart from the record
//! (entrance_keys.rs), so no record, push, decision, transcript or tool result carries it. Only two things read it back:
//! the endpoint, which checks a request against it (`authorize_client_key`), and the owner, who asks the page to show it
//! (`client_key`). An agent that made the key, the Assistant included, never sees it: it tells the owner to open the page.
//!
//! The limit is the library's: an agent of a chain runs as the same OS user as this server, so it can read what the server
//! reads (ADR-0004). Sealing protects the database file and its backups, not the host from an agent that tries.

use std::sync::Arc;

use model::{Actor, ClientKey, Time};
use secrets::Sealed;
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;

use crate::Keys;
use chains::chain_core::{first_line, new_id};
use chains::request::Service;
use chains::request::{Done, Request};
use chains::{Error, Result};

const DEFAULT_CHAINS_PER_DAY: i64 = 3;
const DEFAULT_ACTIONS_PER_HOUR: i64 = 60;
const CLIENT_KEY_NOTE_MAX: usize = 200;
/// How seldom a key's last use is stored again: the page says when it was last used to the minute.
pub const LAST_USED_EVERY: time::Duration = time::Duration::MINUTE;

/// What every client key starts with, so a secret scanner knows one.
const CLIENT_KEY_PREFIX: &str = "ck_";

/// What a key is for and what its holder may do: what update_client_key replaces. A limit left out takes the owner's
/// default.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClientKeySettings {
    /// What the agent that holds it is for, in the owner's words.
    pub note: String,
    /// `work` chains it may start a day, counted on the server's day; zero starts none, left out: 3.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chains_per_day: Option<i64>,
    /// Acts that start or steer it may make an hour; default 60.
    pub actions_per_hour: i64,
}

/// A key as the Assistant's tools take it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClientKeyInput {
    /// What the key is called, lowercase letters, digits and dashes, at most 40, starting with a letter or digit; it never
    /// changes.
    pub name: String,
    #[serde(flatten)]
    pub settings: ClientKeySettings,
}

/// Whether name is what a key may be called (`^[a-z0-9][a-z0-9-]{0,39}$`): it is shown beside every act it makes.
fn is_client_key_name(name: &str) -> bool {
    let word = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    let b = name.as_bytes();
    (1..=40).contains(&b.len())
        && b.first().copied().is_some_and(word)
        && b.iter().skip(1).all(|c| word(*c) || *c == b'-')
}

/// Names the key a sealed value belongs to, so a value moved to another key does not open there.
fn client_key_aad(id: &str) -> String {
    format!("client/{id}")
}

fn no_client_key(id: &str) -> Error {
    Error::refused(format!("no client key {id:?}: list_client_keys shows the ids"))
}

/// v, else fallback when v is zero.
fn or_limit(v: i64, fallback: i64) -> i64 {
    if v == 0 { fallback } else { v }
}

/// The built-in agent that changes code and publishes it.
pub const ENGINEER_AGENT: &str = "engineer";

/// What the Chair of a chain a client key started is told when it hires one that changes code: a client's chain publishes
/// nothing (ADR-0009, ADR-0113). An agent the owner defined under another name is not
/// caught, and an agent's shell can still push (ADR-0004).
pub const CLIENT_ENGINEER: &str = "a chain started through a client key changes no code: hire no engineer";

impl ClientKeySettings {
    /// k with the settings in checked: the owner's defaults filled in.
    fn settings_on(&self, mut k: ClientKey) -> Result<ClientKey> {
        if self.chains_per_day.is_some_and(|n| n < 0) || self.actions_per_hour < 0 {
            return Err(Error::refused("chainsPerDay and actionsPerHour are zero or more"));
        }
        k.chains_per_day = self.chains_per_day.unwrap_or(DEFAULT_CHAINS_PER_DAY);
        k.actions_per_hour = or_limit(self.actions_per_hour, DEFAULT_ACTIONS_PER_HOUR);
        k.note = first_line(self.note.trim(), CLIENT_KEY_NOTE_MAX);
        Ok(k)
    }
}

/// k as by changed it at now.
fn changed_by(mut k: ClientKey, by: Actor, now: Time) -> ClientKey {
    k.by = by;
    k.updated_at = now;
    k
}

/// What a change to a stored key makes of it, and the new sealed value it stores with it, if any.
type KeyChange = (ClientKey, Option<Sealed>);

impl Keys {
    /// Makes a key from the owner's words, with a value of its own, by who asked, and tells the channels.
    pub async fn create_client_key(&self, input: ClientKeyInput, by: Actor) -> Result<ClientKey> {
        let name = input.name.trim().to_lowercase();
        if !is_client_key_name(&name) {
            return Err(Error::refused(format!(
                "the name {:?} is not one: use lowercase letters, digits and dashes, at most 40, starting with a letter or digit, e.g. doctor",
                input.name
            )));
        }
        let now = self.now();
        let k = ClientKey {
            id: new_id("ck_"),
            name,
            by,
            created_at: now,
            updated_at: now,
            ..ClientKey::default()
        };
        let k = input.settings.settings_on(k)?;
        let sealed = self.seal_new_key(CLIENT_KEY_PREFIX, &client_key_aad(&k.id)).await?;
        let name = k.name.clone();
        let k = match self.store.update(move |tx| tx.create_client_key(k, &sealed)).await {
            Err(store::Error::ClientKeyName) => {
                return Err(Error::refused(format!(
                    "a client key named {name:?} exists already: list_client_keys shows it"
                )));
            }
            other => other?,
        };
        self.notify.client_key_updated(&k);
        Ok(k)
    }

    /// Stores what change makes of key id, read and written in one transaction so no change is lost, and tells the
    /// channels. A change that returns a sealed value stores it with the record.
    async fn change_client_key<F>(&self, id: &str, change: F) -> Result<ClientKey>
    where
        F: FnOnce(&store::Reader<'_>, ClientKey) -> Result<KeyChange> + Send + 'static,
    {
        let id = id.to_owned();
        let saved = self
            .store
            .update(move |tx| {
                let cur = match tx.client_key(&id) {
                    Err(store::Error::NotFound) => return Err(no_client_key(&id)),
                    other => other?,
                };
                let (next, sealed) = change(tx, cur)?;
                Ok::<_, Error>(match sealed {
                    Some(sealed) => tx.rotate_client_key(next, &sealed)?,
                    None => tx.put_client_key(next)?,
                })
            })
            .await?;
        self.notify.client_key_updated(&saved);
        Ok(saved)
    }

    /// Replaces a key's settings. Its name never changes.
    pub(crate) async fn update_client_key(
        &self,
        id: &str,
        settings: ClientKeySettings,
        by: Actor,
    ) -> Result<ClientKey> {
        let now = self.now();
        self.change_client_key(id, move |_, k| {
            Ok((changed_by(settings.settings_on(k)?, by, now), None))
        })
        .await
    }

    /// Opens nothing for a key until it is resumed; resuming opens it again. Its value stays, and so do the chains it
    /// started.
    pub async fn pause_client_key_as(&self, id: &str, paused: bool, by: Actor) -> Result<ClientKey> {
        let now = self.now();
        self.change_client_key(id, move |_, mut k| {
            k.paused = paused;
            Ok((changed_by(k, by, now), None))
        })
        .await
    }

    /// Gives a key a new value: the old one stops opening the endpoint at once.
    pub async fn rotate_client_key(&self, id: &str, by: Actor) -> Result<ClientKey> {
        let sealed = self.seal_new_key(CLIENT_KEY_PREFIX, &client_key_aad(id)).await?;
        let now = self.now();
        self.change_client_key(id, move |_, k| Ok((changed_by(k, by, now), Some(sealed))))
            .await
    }

    /// Removes a key and its value: nothing opens the endpoint with it again. The chains it started go on, and what it did
    /// stays on them.
    pub async fn delete_client_key(&self, id: &str) -> Result<()> {
        let key = id.to_owned();
        match self.store.read(move |r| r.client_key(&key)).await {
            Err(store::Error::NotFound) => return Err(no_client_key(id)),
            other => other?,
        };
        let key = id.to_owned();
        self.store.update(move |tx| tx.delete_client_key(&key)).await?;
        self.notify.client_key_deleted(id);
        Ok(())
    }

    /// The owner shutting a key until he resumes it.
    pub async fn pause_client_key(self: &Arc<Self>, req: Request, id: String) -> Result<Done> {
        let by = req.by.clone();
        self.act_done(req, "PauseClientKey", move |o| async move {
            o.pause_client_key_as(&id, true, by).await.map(|_| ())
        })
        .await
    }

    /// The owner opening a paused key again.
    pub async fn resume_client_key(self: &Arc<Self>, req: Request, id: String) -> Result<Done> {
        let by = req.by.clone();
        self.act_done(req, "ResumeClientKey", move |o| async move {
            o.pause_client_key_as(&id, false, by).await.map(|_| ())
        })
        .await
    }

    /// Which key the presented value is: one that exists, is not paused and whose value it is. Every way of failing gives
    /// the same answer, so a holder learns nothing about which keys exist. An error is the server's: the store could not be
    /// read. A value that cannot be opened is the owner's to fix by rotating that key, and the other keys still open. A
    /// key that opens is noted as used from addr.
    pub async fn authorize_client_key(&self, presented: &str, addr: &str) -> Result<Option<ClientKey>> {
        let keys = self
            .store
            .read(|r| {
                r.client_keys()?
                    .into_iter()
                    .map(|k| r.sealed_client_key(&k.id).map(|s| (k, s)))
                    .collect::<store::Result<Vec<_>>>()
            })
            .await?;
        let mut matched: Option<ClientKey> = None;
        for (k, sealed) in keys {
            let value = match self.opened_key(&client_key_aad(&k.id), &sealed) {
                Ok(value) => value,
                Err(err) => {
                    tracing::error!(key = %k.id, name = %k.name, %err, "a client key cannot be opened");
                    continue;
                }
            };
            // Every key is compared, so the time taken does not tell how many were tried before the right one.
            if bool::from(value.as_bytes().ct_eq(presented.as_bytes())) {
                matched = Some(k);
            }
        }
        match matched {
            Some(k) if !k.paused => Ok(Some(self.note_used(k, addr).await)),
            _ => Ok(None),
        }
    }

    /// Stores that key k opened the endpoint from addr, at most once a minute, and returns it as stored. A failure to store
    /// it is logged: the holder is let in all the same.
    async fn note_used(&self, k: ClientKey, addr: &str) -> ClientKey {
        let now = self.now();
        let recent = k
            .last_used_at
            .and_then(|last| last.0.checked_add(LAST_USED_EVERY))
            .is_some_and(|next| now.0 < next);
        if recent && k.last_addr == addr {
            return k;
        }
        let at = addr.to_owned();
        match self
            .change_client_key(&k.id, move |_, mut cur| {
                cur.last_used_at = Some(now);
                cur.last_addr = at;
                Ok((cur, None))
            })
            .await
        {
            Ok(saved) => saved,
            Err(err) => {
                tracing::error!(key = %k.id, %err, "a client key's use is not stored");
                k
            }
        }
    }

    /// Key id's value, for the owner who asked to see it: the one place a channel is given it. It is read, never pushed,
    /// and the server's log says who asked. No one but the owner is given it: an agent that asks is told to open the page.
    pub async fn client_key(&self, id: &str, by: &Actor) -> Result<String> {
        chains::owner_acts::for_owner(chains::owner_acts::ACT_SEE_KEY, by)?;
        let key = id.to_owned();
        let sealed = match self.store.read(move |r| r.sealed_client_key(&key)).await {
            Err(store::Error::NotFound) => return Err(no_client_key(id)),
            other => other?,
        };
        let value = self.opened_key(&client_key_aad(id), &sealed).map_err(|e| {
            Error::refused(format!(
                "the key {e}; or rotate it, and put the new one where the old one was"
            ))
        })?;
        tracing::info!(key = %id, by = %by.kind, who = %by.who, "client key shown");
        Ok(value)
    }
}
