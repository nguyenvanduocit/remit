//! The Assistant's client key tools: how the owner hands an outside agent a key, limits it and takes it back, by telling
//! the Assistant. Each is the orchestrator's own action (client_keys.rs), taken as the Assistant for him, so the owner sees
//! the key on the Client keys page at once and who made it. None returns a key's value: it is shown on that page, to him
//! alone, and an agent never sees it. Deleting or pausing a key stops its next act; the chains it started go on until the
//! owner pauses them.

use std::sync::Arc;

use model::{Actor, Time, by_client};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Keys;
use crate::client_keys::{ClientKeyInput, ClientKeySettings};
use chains::tools::{Effect, NoInput, Tools, as_json};

/// A key as create_client_key takes it.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CreateClientKeyInput {
    /// what the key is called, lowercase letters, digits and dashes, at most 40, starting with a letter or digit; it never changes
    name: String,
    /// what the agent that holds it is for, in the owner's words, e.g. 'the doctor that evaluates Remit's
    #[serde(default)]
    note: String,
    /// work chains it may start a day, counted on the server's day; 0 starts none; leave out for the default, 3
    #[serde(default)]
    chains_per_day: Option<i64>,
    /// acts that start or steer it may make an hour; default 60
    #[serde(default)]
    actions_per_hour: i64,
}

/// A key's settings as update_client_key replaces them.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UpdateClientKeyInput {
    /// client key id (ck_…), as list_client_keys shows it
    id: String,
    /// what the agent that holds it is for, in the owner's words, e.g. 'the doctor that evaluates Remit's
    #[serde(default)]
    note: String,
    /// work chains it may start a day, counted on the server's day; 0 starts none; leave out for the default, 3
    #[serde(default)]
    chains_per_day: Option<i64>,
    /// acts that start or steer it may make an hour; default 60
    #[serde(default)]
    actions_per_hour: i64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ClientKeyIdInput {
    /// client key id (ck_…), as list_client_keys shows it
    id: String,
}

/// A client key as the Assistant reads it: no value, what it did, and its settings in the shape update_client_key
/// takes.
#[derive(Debug, Serialize)]
struct ClientKeyRecord {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    paused: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_used_at: Option<Time>,
    #[serde(skip_serializing_if = "String::is_empty")]
    last_addr: String,
    /// Newest first, with their status.
    #[serde(rename = "chains_started", skip_serializing_if = "Vec::is_empty")]
    chains: Vec<String>,
    #[serde(flatten)]
    settings: ClientKeySettings,
}

const CLIENT_KEY_ON_THE_PAGE: &str = "Its value is on the Client keys page (under Integrations): tell the owner to open it and copy the key there into a file of mode 0600 for the agent. You are never shown it, and it never goes in a message, a prompt or an argument.";

impl Keys {
    pub fn add_client_key_tools(self: &Arc<Self>, t: &mut Tools, by: &Actor) {
        t.serving(self).add(
            "list_client_keys", Effect::Reads,
            "The client keys the owner handed to outside agents: each with its limits (work chains a day, acts an hour), whether it is paused, when and from where it was last used and the chains it started, in the shape update_client_key takes. A value is never shown to you.",
            |o, _: NoInput| async move {
                let keys = o.store.read(|r| r.client_keys()).await?;
                let chains = o.store.read(|r| r.chains()).await?;
                let recs: Vec<ClientKeyRecord> = keys
                    .into_iter()
                    .map(|k| {
                        let by = by_client(&k.id);
                        ClientKeyRecord {
                            chains: chains
                                .iter()
                                .filter(|c| c.by.as_ref() == Some(&by))
                                .map(|c| format!("{} ({})", c.id, c.status()))
                                .collect(),
                            last_used_at: k.last_used_at,
                            id: k.id,
                            name: k.name,
                            paused: k.paused,
                            last_addr: k.last_addr,
                            settings: ClientKeySettings {
                                note: k.note,
                                chains_per_day: Some(k.chains_per_day),
                                actions_per_hour: k.actions_per_hour,
                            },
                        }
                    })
                    .collect();
                as_json(&recs)
            },
        );
        let create_by = by.clone();
        t.serving(self).add(
            "create_client_key", Effect::Writes,
            format!(
                "Hand an outside agent a key to Remit, when the owner asked for one: it lets that agent read what he sees, start work chains (which hire no engineer and publish nothing) within the limits you set, and pause, finish, stop or message what runs. It never resumes, answers, merges, or changes a credential, key, event signal or definition. Name it as he would say it (lowercase, dashes). Leave a limit out for the default: 3 chains a day, 60 acts an hour; 0 chains a day starts none. {CLIENT_KEY_ON_THE_PAGE}"
            ),
            move |o, i: CreateClientKeyInput| {
                let by = create_by.clone();
                async move {
                    let input = ClientKeyInput {
                        name: i.name,
                        settings: ClientKeySettings {
                            note: i.note,
                            chains_per_day: i.chains_per_day,
                            actions_per_hour: i.actions_per_hour,
                        },
                    };
                    let k = o.create_client_key(input, by).await?;
                    Ok(format!("Made client key {}, {:?}. {CLIENT_KEY_ON_THE_PAGE}", k.id, k.name))
                }
            },
        );
        let update_by = by.clone();
        t.serving(self).add(
            "update_client_key", Effect::Writes,
            "Replace a client key's settings: what it is for and its limits (all of them: start from list_client_keys and change what he asked, so the rest stays). Its name never changes.",
            move |o, i: UpdateClientKeyInput| {
                let by = update_by.clone();
                async move {
                    let settings = ClientKeySettings {
                        note: i.note,
                        chains_per_day: i.chains_per_day,
                        actions_per_hour: i.actions_per_hour,
                    };
                    let k = o.update_client_key(&i.id, settings, by).await?;
                    Ok(format!("Updated client key {}, {:?}.", k.id, k.name))
                }
            },
        );
        let rotate_by = by.clone();
        t.serving(self).add(
            "rotate_client_key", Effect::Writes,
            format!(
                "Give a client key a new value, when the owner asks (it leaked): the old one stops working at once, so whoever holds it must be given the new one. {CLIENT_KEY_ON_THE_PAGE}"
            ),
            move |o, i: ClientKeyIdInput| {
                let by = rotate_by.clone();
                async move {
                    let k = o.rotate_client_key(&i.id, by).await?;
                    Ok(format!(
                        "Rotated the value of client key {}, {:?}: the old one no longer works. {CLIENT_KEY_ON_THE_PAGE}",
                        k.id, k.name
                    ))
                }
            },
        );
        let pause_by = by.clone();
        t.serving(self).add(
            "pause_client_key", Effect::Writes,
            "Pause a client key for the owner: it opens nothing until it resumes. The chains it started go on; pause them with pause_chain if he asks.",
            move |o, i: ClientKeyIdInput| {
                let by = pause_by.clone();
                async move {
                    let k = o.pause_client_key_as(&i.id, true, by).await?;
                    Ok(format!("Paused client key {}, {:?}.", k.id, k.name))
                }
            },
        );
        let resume_by = by.clone();
        t.serving(self).add(
            "resume_client_key",
            Effect::Writes,
            "Resume a paused client key for the owner: its value opens the endpoint again.",
            move |o, i: ClientKeyIdInput| {
                let by = resume_by.clone();
                async move {
                    let k = o.pause_client_key_as(&i.id, false, by).await?;
                    Ok(format!("Resumed client key {}, {:?}.", k.id, k.name))
                }
            },
        );
        t.serving(self).add(
            "delete_client_key", Effect::Writes,
            "Delete a client key the owner named, with its value: nothing opens the endpoint with it again. The chains it started go on, and what it did stays on them.",
            |o, i: ClientKeyIdInput| async move {
                o.delete_client_key(&i.id).await?;
                Ok(format!("Deleted client key {}.", i.id))
            },
        );
    }
}
