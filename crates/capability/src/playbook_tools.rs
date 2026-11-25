//! The playbook tools. Every Chair and the Assistant read the playbooks; the owner never edits one himself: he tells the
//! Assistant what he wants, and the Chair of an `author` chain turns it into the writes (ADR-0113).

use std::sync::Arc;

use model::{DecisionKind, Playbook, Seed};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Capabilities;
use crate::agent_tools::Author;
use chains::Error;
use chains::tools::NoInput;
use chains::tools::{Effect, SelfCard, Tools, as_json, is_zero_rev};

/// A seed of a playbook in the shape the tools take.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SeedInput {
    /// the id of the agent this card is, one of list_agent_definitions, or a short word for a plain agent (every tool,
    /// nothing from the library), e.g. research, architect, engineer
    pub agent: String,
    /// display name of the card; empty uses the agent
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// the agent's task: the goal, the evidence it must bring, what it delivers
    pub brief: String,
    /// a model id the Chair may hire on (hire_agent lists them); empty uses the Chair's own model
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model: String,
    /// its default effort: low, medium, high, xhigh or max; empty uses the hiring Chair's
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub effort: String,
}

/// A playbook in the shape the tools take, so what read_playbook returns can go straight back into update_playbook.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlaybookInput {
    /// lowercase letters, digits and dashes, e.g. payment-bug-hunt; it never changes after creation
    pub id: String,
    /// short display name
    pub name: String,
    /// one line for the playbook list, in the owner's language
    pub description: String,
    /// the kind of work it is for, in lowercase words like engineering or data: its column on the Playbooks page. Reuse a
    /// group list_playbooks already shows; empty is general. A built-in playbook's is fixed
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    /// the playbook's text, read by a Chair whose work it fits: how this kind of work goes well, what to prioritise, what
    /// to avoid, which decisions to take and which to leave to people. It binds no one
    pub brief: String,
    /// the agents it suggests, in order; a Chair may hire them, others, or none
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seeds: Vec<SeedInput>,
    /// the revision read_playbook returned: update_playbook writes over that revision only, so a change made since is never lost.
    /// Leave it out to create
    #[serde(default, skip_serializing_if = "is_zero_rev")]
    pub rev: i64,
}

/// A playbook, named.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PlaybookIdInput {
    /// playbook id, as list_playbooks shows it
    id: String,
}

impl PlaybookInput {
    fn playbook(self) -> Playbook {
        Playbook {
            id: self.id,
            name: self.name,
            description: self.description,
            group: self.group,
            brief: self.brief,
            rev: self.rev,
            seeds: self
                .seeds
                .into_iter()
                .map(|s| Seed {
                    agent: s.agent,
                    name: s.name,
                    brief: s.brief,
                    model: s.model,
                    effort: s.effort,
                })
                .collect(),
            ..Playbook::default()
        }
    }
}

/// A playbook in the shape the tools take.
fn playbook_input(p: Playbook) -> PlaybookInput {
    PlaybookInput {
        id: p.id,
        name: p.name,
        description: p.description,
        group: p.group,
        brief: p.brief,
        rev: p.rev,
        seeds: p
            .seeds
            .into_iter()
            .map(|s| SeedInput {
                agent: s.agent,
                name: s.name,
                brief: s.brief,
                model: s.model,
                effort: s.effort,
            })
            .collect(),
    }
}

impl Capabilities {
    /// The tools that write the playbooks, as me: an `author` chain's Chair's.
    pub fn add_playbook_tools(self: &Arc<Self>, t: &mut Tools, me: SelfCard) {
        self.add_playbook_write_tools(t, &Self::author_of(me, DecisionKind::Playbook));
    }

    /// The tools that read the playbooks: every Chair's, and the Assistant's.
    pub fn add_playbook_read_tools(self: &Arc<Self>, t: &mut Tools) {
        t.serving(self).add(
            "list_playbooks", Effect::Reads,
            "Every playbook: id, name, description, group, number of suggested agents, whether it is built in and whether the owner has changed it. A playbook is reference: read one with read_playbook when it fits your work, and take what helps.",
            |o, _: NoInput| async move {
                #[derive(Serialize)]
                #[serde(rename_all = "PascalCase")]
                struct Row {
                    #[serde(rename = "ID")]
                    id: String,
                    name: String,
                    description: String,
                    group: String,
                    seeds: usize,
                    builtin: bool,
                    edited: bool,
                }
                let all = o.store.read(|r| r.playbooks()).await?;
                let rows: Vec<Row> = all
                    .into_iter()
                    .map(|p| Row {
                        seeds: p.seeds.len(),
                        id: p.id,
                        name: p.name,
                        description: p.description,
                        group: p.group,
                        builtin: p.builtin,
                        edited: p.edited,
                    })
                    .collect();
                as_json(&rows)
            },
        );
        t.serving(self).add(
            "read_playbook",
            Effect::Reads,
            "One playbook in full, in the shape create_playbook and update_playbook take: its brief and the agents it suggests. It binds no one.",
            |o, i: PlaybookIdInput| async move {
                let key = i.id.clone();
                let p = o
                    .store
                    .read(move |r| r.playbook(&key))
                    .await
                    .map_err(|e| Error::refused(format!("playbook {:?}: {e}", i.id)))?;
                as_json(&playbook_input(p))
            },
        );
    }

    /// The tools that change the playbooks; au records each change where the owner sees it.
    fn add_playbook_write_tools(self: &Arc<Self>, t: &mut Tools, au: &Author) {
        let a = au.clone();
        t.serving(self).add(
            "create_playbook",
            Effect::Writes,
            "Create a new playbook. An id already in use is refused. The owner sees it in the playbook list at once.",
            move |o, i: PlaybookInput| {
                let au = a.clone();
                async move {
                    let by = au.by(&o).await?;
                    let written = o.create_playbook(&by, i.playbook()).await;
                    au.answer(&o, written, |p| {
                        (
                            format!("Create playbook {}.", p.name),
                            format!("Created playbook {} ({} seed agents).", p.id, p.seeds.len()),
                        )
                    })
                    .await
                }
            },
        );
        let a = au.clone();
        t.serving(self).add(
            "update_playbook", Effect::Writes,
            "Replace an existing playbook with this version, made from what read_playbook returned (its rev included): every field is replaced, so keep what the owner did not ask to change. The id stays. A playbook changed since that read is not written over: the answer says so, and you read it again and make the change to what it holds now.",
            move |o, i: PlaybookInput| {
                let au = a.clone();
                async move {
                    let by = au.by(&o).await?;
                    let written = o.update_playbook(&by, i.playbook()).await;
                    au.answer(&o, written, |p| {
                        (
                            format!("Update playbook {}.", p.name),
                            format!("Updated playbook {} ({} seed agents).", p.id, p.seeds.len()),
                        )
                    })
                    .await
                }
            },
        );
        let a = au.clone();
        t.serving(self).add(
            "delete_playbook", Effect::Writes,
            "Delete a playbook the owner named. A built-in playbook cannot be deleted: reset_playbook puts it back to its original instead. Chains already made from it are not affected.",
            move |o, i: PlaybookIdInput| {
                let au = a.clone();
                async move {
                    let by = au.by(&o).await?;
                    o.delete_playbook(&by, &i.id).await?;
                    au.record(&o, &format!("Delete playbook {}.", i.id)).await?;
                    Ok(format!("Deleted playbook {}.", i.id))
                }
            },
        );
        let a = au.clone();
        t.serving(self).add(
            "reset_playbook",
            Effect::Writes,
            "Put a built-in playbook back to the version that ships with Remit, discarding the owner's changes to it.",
            move |o, i: PlaybookIdInput| {
                let au = a.clone();
                async move {
                    let by = au.by(&o).await?;
                    let p = o.reset_playbook(&by, &i.id).await?;
                    au.record(&o, &format!("Reset playbook {} to its original.", p.name))
                        .await?;
                    Ok(format!("Reset playbook {} to its original.", p.id))
                }
            },
        );
    }
}
