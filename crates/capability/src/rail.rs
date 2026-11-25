//! What moves a rail. An agent that holds the `agents` or the `library` capability writes records on the owner's word.
//! Most of what it writes is configuration. Instructions that outlive the chain that wrote them are more: an agent
//! definition's, which join the system prompt of every card hired from it, and a skill's, which an agent loads. Text an
//! injected turn writes there would steer every later hire, so a write that changes one waits for the owner's answer
//! (change_item.rs) instead of taking effect. A playbook is reference and moves none (ADR-0113).
//!
//! These are pure: each says, from the record as stored and as it would be written, which of those the write changes.

use std::collections::BTreeMap;

use model::{Agent, ChangedField, Skill};
use serde::Serialize;

use crate::service::Capabilities;
use chains::Result;
use chains::chain_core::optional;
use chains::changes::doc_of;
use model::Actor;
use model::ActorKind;
use model::ChangeKind;

/// A field of a record as a rail check names it; its before is empty when the record did not exist.
fn rail_field(name: &str, before: &impl Serialize, after: &impl Serialize, existed: bool) -> ChangedField {
    ChangedField {
        name: name.to_owned(),
        before: if existed { json_text(before) } else { String::new() },
        after: json_text(after),
    }
}

/// v as library.rs writes a record's field to compare: compact JSON with sorted keys.
fn json_text(v: &impl Serialize) -> String {
    serde_json::to_value(v)
        .map(|v| library::json_text(&v))
        .unwrap_or_default()
}

/// The fields of agent after, as it would be written, that move a rail against before, the agent as stored (none when it
/// is new): its instructions.
pub fn agent_rail(before: Option<&Agent>, after: &Agent) -> Vec<ChangedField> {
    let was = before.map(|a| a.loadout.instructions.as_str()).unwrap_or_default();
    if was == after.loadout.instructions {
        return Vec::new();
    }
    vec![rail_field(
        "instructions",
        &was,
        &after.loadout.instructions,
        before.is_some(),
    )]
}

/// The fields of skill after, as it would be written, that move a rail against before, the skill as stored (none when it
/// is new): its description, which tells an agent when to load it, and the files that differ, each as it is and would
/// be.
pub fn skill_rail(before: Option<&Skill>, after: &Skill) -> Vec<ChangedField> {
    let empty = BTreeMap::new();
    let (was_description, was_files) = before.map_or(("", &empty), |s| (s.description.as_str(), &s.files));
    let mut out = Vec::new();
    if was_description != after.description {
        out.push(rail_field(
            "description",
            &was_description,
            &after.description,
            before.is_some(),
        ));
    }
    let (b, a) = (differing(was_files, &after.files), differing(&after.files, was_files));
    if !b.is_empty() || !a.is_empty() {
        out.push(rail_field("files", &b, &a, before.is_some()));
    }
    out
}

/// The files of one whose path is not in other with the same text.
fn differing<'a>(one: &'a BTreeMap<String, String>, other: &BTreeMap<String, String>) -> BTreeMap<&'a str, &'a str> {
    one.iter()
        .filter(|(p, text)| other.get(*p) != Some(*text))
        .map(|(p, text)| (p.as_str(), text.as_str()))
        .collect()
}

impl Capabilities {
    /// Asks the owner before an agent writes agent definition a when it moves a rail, and says so with
    /// `Error::WaitingForOwner`. It is Ok when the write may go ahead, and when a cannot be written at all: the write
    /// itself refuses it, in words the agent can act on.
    pub(crate) async fn hold_agent(&self, by: &Actor, a: &Agent) -> Result<()> {
        if by.kind != ActorKind::Agent {
            return Ok(());
        }
        let Ok(a) = self.prepared_agent(a.clone()).await else {
            return Ok(());
        };
        let id = a.id.clone();
        let cur = self.store.read(move |r| optional(r.agent(&id))).await?;
        let fields = agent_rail(cur.as_ref(), &a);
        self.hold_change(by, ChangeKind::Agent, &a.id, doc_of(Some(&a))?, fields)
            .await
    }
    /// `hold_agent` for a skill.
    pub(crate) async fn hold_skill(&self, by: &Actor, s: &Skill) -> Result<()> {
        if by.kind != ActorKind::Agent {
            return Ok(());
        }
        let Ok(s) = self.prepared_skill(s.clone()).await else {
            return Ok(());
        };
        let name = s.name.clone();
        let cur = self.store.read(move |r| optional(r.skill(&name))).await?;
        let fields = skill_rail(cur.as_ref(), &s);
        self.hold_change(by, ChangeKind::Skill, &s.name, doc_of(Some(&s))?, fields)
            .await
    }
}
