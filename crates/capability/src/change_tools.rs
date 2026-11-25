//! The tools that read what changed. The Chair that writes the playbooks, agents and library reads them to know why a
//! record is as it is, and what it was, before it writes it back; the Assistant reads them to answer the owner who asks.

use std::sync::Arc;

use model::{Change, ChangeKind, ChangedField, Time};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Capabilities;
use chains::outbox::actor_words;
use chains::tools::{Effect, Tools, as_json};
use chains::{Error, Result};

/// The record whose changes are read.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ChangesInput {
    /// agent, skill, playbook, grant, event_signal, sensor, extension, objective, employee or forget
    kind: String,
    /// the record's name, or its id for an agent or a playbook
    name: String,
}

/// One change, named.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ChangeIdInput {
    /// a change's id, as list_changes shows it
    id: String,
}

/// A change as list_changes shows it: who and when in words, and the fields that differ.
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct ChangeRow {
    #[serde(rename = "ID")]
    id: String,
    /// to the second, in RFC 3339
    at: Time,
    by: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    approved_by: String,
    fields: Vec<ChangedField>,
}

/// How many of a record's changes a channel is given, newest first.
const CHANGES_SHOWN: i64 = 20;

impl Capabilities {
    /// Gives a card the tools that read what changed in a record.
    pub fn add_change_read_tools(self: &Arc<Self>, t: &mut Tools) {
        t.serving(self).add(
            "list_changes", Effect::Reads,
            "What was changed in one agent, skill, playbook or grant (named chain/source/connection), newest first (the last 20): when, who wrote it, who let it through when it waited for the owner, and the fields that differ, each as it was and as it became. A record that was deleted shows its deletion.",
            |o, i: ChangesInput| async move {
                let rows: Vec<ChangeRow> = o.changes(change_kind_of(&i.kind)?, &i.name).await?.into_iter().map(change_row).collect();
                as_json(&rows)
            },
        );
        t.serving(self).add(
            "read_change", Effect::Reads,
            "One change in full: the record before and after, as it is stored. To put a record back as it was, write what `before` says with update_skill, update_mcp_server, update_agent_definition or update_playbook; the record's own tools take the fields under the names read_skill, read_mcp_server, read_agent_definition and read_playbook give them.",
            |o, i: ChangeIdInput| async move {
                let id = i.id.clone();
                let c = o
                    .store
                    .read(move |r| r.change(&id))
                    .await
                    .map_err(|e| Error::refused(format!("change {:?}: {e}", i.id)))?;
                as_json(&c)
            },
        );
    }

    /// The writes of one record, newest first: what the browser shows beside the record and list_changes tells an
    /// agent. A kind that is no record with changes is refused, in words an agent can act on.
    pub async fn changes(&self, kind: ChangeKind, name: &str) -> Result<Vec<Change>> {
        if kind == ChangeKind::Capability {
            return Err(Error::refused(
                "a capability given to a chain is a decision of the chain, not a record with changes: read the chain's \
                 decisions",
            ));
        }
        let name = name.to_owned();
        Ok(self.store.read(move |r| r.changes(&kind, &name, CHANGES_SHOWN)).await?)
    }
}

/// A change as list_changes shows it.
fn change_row(c: Change) -> ChangeRow {
    ChangeRow {
        id: c.id,
        at: Time(c.at.0.replace_nanosecond(0).unwrap_or(c.at.0)),
        by: actor_words(&c.by),
        approved_by: c.approved_by.as_ref().map(actor_words).unwrap_or_default(),
        fields: c.fields,
    }
}

/// The kind of record a tool names, in words an agent can act on when it names none.
fn change_kind_of(s: &str) -> Result<ChangeKind> {
    s.parse().map_err(|_| {
        Error::refused(format!(
            "kind {s:?}: the records that have changes are agent, skill, playbook, grant, event_signal, sensor, \
             extension, objective, employee, forget, promote, project and key_result"
        ))
    })
}
