//! The Assistant's worker key tools: making a code a mentor works from on the owner's words, which codes a mentor holds,
//! and taking one back (a lost phone, a host no longer theirs). None shows a code's value: the owner reads it on the
//! mentor's row and passes it on (ADR-0060).

use std::sync::Arc;

use model::{Actor, Time};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Keys;
use chains::tools::{Effect, Tools, as_json};
use chains::{Error, Result};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CreateWorkerKeyInput {
    /// the id of the mentor, as list_mentors shows it
    mentor: String,
    /// what the code is for, in the owner's words, at most 100 characters: Lan's phone
    #[serde(default)]
    note: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListWorkerKeysInput {
    /// the id of the mentor, as list_mentors shows it
    mentor: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RevokeWorkerKeyInput {
    /// the id of one code (wkey_…), as list_worker_keys shows it
    #[serde(default)]
    id: String,
    /// the id of a mentor: every code of theirs stops; leave id out
    #[serde(default)]
    mentor: String,
}

/// A code of a mentor's as the Assistant reads it: no value.
#[derive(Debug, Serialize)]
struct WorkerKeyRecord {
    id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    note: String,
    created_at: Time,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_used_at: Option<Time>,
    #[serde(skip_serializing_if = "String::is_empty")]
    last_addr: String,
}

impl Keys {
    /// Gives the Assistant, acting as by, the tools that make, list and revoke the codes mentors work from.
    pub fn add_worker_key_tools(self: &Arc<Self>, t: &mut Tools, by: &Actor) {
        let create_by = by.clone();
        t.serving(self).add(
            "create_worker_key", Effect::Writes,
            "Make a new code a mentor works from, when the owner asks: one per device they work from, as many as they need. You are never shown its value: tell the owner to open the mentor's row in Mentors (on Crew), copy its sign-in link there and pass it on; the mentor opens it, or pastes the code on Remit's sign-in page. It works until it is revoked or they are removed.",
            move |o, i: CreateWorkerKeyInput| {
                let by = create_by.clone();
                async move {
                    let k = o.create_worker_key(&i.mentor, &i.note, &by).await?;
                    Ok(format!(
                        "Made code {} for {}. Its value shows only to the owner, on the mentor's row in Mentors.",
                        k.id, k.mentor
                    ))
                }
            },
        );
        t.serving(self).add(
            "list_worker_keys", Effect::Reads,
            "The codes a mentor works from: each code's id, the owner's note of what it is for, when it was made, and when and from where it was last used. A code's value is never shown.",
            |o, i: ListWorkerKeysInput| async move {
                let keys = o.store.read(move |r| r.worker_keys(&i.mentor)).await?;
                let recs: Vec<WorkerKeyRecord> = keys
                    .into_iter()
                    .map(|k| WorkerKeyRecord {
                        last_used_at: k.last_used_at,
                        id: k.id,
                        note: k.note,
                        created_at: k.created_at,
                        last_addr: k.last_addr,
                    })
                    .collect();
                as_json(&recs)
            },
        );
        t.serving(self).add(
            "revoke_worker_key", Effect::Writes,
            "Stop a mentor's code when the owner asks (a lost phone, a host no longer theirs): give id for one code, or mentor for every code of theirs. The code stops working at once and its connection closes; a new one is made with create_worker_key.",
            |o, i: RevokeWorkerKeyInput| async move { o.revoke_worker_keys(i).await },
        );
    }

    /// What revoke_worker_key does: one code by its id, or every code of a mentor.
    async fn revoke_worker_keys(&self, i: RevokeWorkerKeyInput) -> Result<String> {
        match (i.id.is_empty(), i.mentor.is_empty()) {
            (false, false) => Err(Error::refused(
                "give id for one code or mentor for all of theirs, not both",
            )),
            (false, true) => {
                let id = i.id.clone();
                if let Err(err) = self.store.read(move |r| r.worker_key(&id)).await {
                    return Err(Error::refused(format!("code {}: {err}", i.id)));
                }
                self.revoke_worker_key(&i.id).await?;
                Ok(format!("Revoked code {}.", i.id))
            }
            (true, false) => {
                let mentor = i.mentor.clone();
                let keys = self.store.read(move |r| r.worker_keys(&mentor)).await?;
                for k in &keys {
                    self.revoke_worker_key(&k.id).await?;
                }
                Ok(format!("Revoked {} code(s) of {}.", keys.len(), i.mentor))
            }
            (true, true) => Err(Error::refused(
                "give id for one code, or mentor for every code of theirs",
            )),
        }
    }
}
