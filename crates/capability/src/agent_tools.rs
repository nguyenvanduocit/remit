//! The tools that define agents. The owner never edits an agent himself: he tells an agent what he wants, and the Chair of
//! a chain that holds the `agents` capability (an author chain's) turns it into these calls. They are named
//! *_agent_definition because list_agents is the Chair's tool for the agents working in its chain.

use std::sync::Arc;

use model::{Actor, Agent, DecisionKind, Loadout, Signs, by_agent};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Capabilities;
use chains::change_item::held_words;
use chains::tools::NoInput;
use chains::tools::{Effect, SelfCard, Tools, as_json, is_zero_rev};
use chains::{Error, Result};

/// An agent definition in the shape the tools take, so what read_agent_definition returns can go straight back into
/// update_agent_definition.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AgentInput {
    /// lowercase letters, digits and dashes, e.g. frontend; it never changes after creation
    pub id: String,
    /// short display name
    pub name: String,
    /// one line for the agent list and for the Chair choosing whom to hire, in the owner's language
    pub description: String,
    /// what every session of this agent starts with, added to its system prompt: how it works, what it must always or
    /// never do. Written for the agent, in English like the built-in ones. Not the task a Chair gives it: that is the
    /// brief of its hire
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub instructions: String,
    /// the kind of work it is for, in lowercase words like engineering or customer support: its column on the Agents
    /// page. Reuse a group list_agent_definitions already shows; empty is general. A built-in agent's is fixed
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    /// its default model id (hire_agent lists them); empty uses the hiring Chair's
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model: String,
    /// its default effort: low, medium, high, xhigh or max; empty uses the hiring Chair's
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub effort: String,
    /// the Claude Code tools it may use, e.g. Read, Grep, Glob, Edit, Write, Bash. Leave it out for every tool; send [] for
    /// none. It narrows the agent: a frontend agent needs few
    // An option, so that an agent with no tool at all is told apart from one with every tool: leaving it out is every
    // tool, [] is none, and read_agent_definition says which.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    /// names of library skills it holds (list_library shows them)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    /// review, qc or security: the gate it closes with a verdict (security judges a change that touches a rail the
    /// repository declares in .remit/rails). Only a judging agent sets it; a built-in one's is fixed
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signs: String,
    /// the revision read_agent_definition returned: update_agent_definition writes over that revision only, so a change made since is never lost.
    /// Leave it out to create
    #[serde(default, skip_serializing_if = "is_zero_rev")]
    pub rev: i64,
}

/// An agent, named.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AgentIdInput {
    /// agent id, as list_agent_definitions shows it
    id: String,
}

impl AgentInput {
    fn agent(self) -> Result<Agent> {
        let Ok(signs) = self.signs.parse::<Signs>() else {
            return Err(Error::refused(format!(
                "signs {:?}: use review, qc or security, or leave it out",
                self.signs
            )));
        };
        Ok(Agent {
            id: self.id,
            name: self.name,
            description: self.description,
            group: self.group,
            model: self.model,
            effort: self.effort,
            signs,
            loadout: Loadout {
                instructions: self.instructions,
                tools: self.tools,
                skills: self.skills,
            },
            rev: self.rev,
            ..Agent::default()
        })
    }
}

fn agent_input(a: Agent) -> AgentInput {
    AgentInput {
        id: a.id,
        name: a.name,
        description: a.description,
        group: a.group,
        instructions: a.loadout.instructions,
        model: a.model,
        effort: a.effort,
        tools: a.loadout.tools,
        skills: a.loadout.skills,
        signs: a.signs.to_string(),
        rev: a.rev,
    }
}

/// Says what an agent's tools list means, for a reader that does not tell none from empty.
pub fn tools_label(tools: Option<&[String]>) -> String {
    match tools {
        None => "every tool".to_owned(),
        Some([]) => "none".to_owned(),
        Some(list) => list.join(", "),
    }
}

/// The Chair a write tool is called as: it says who writes, and records what it did on its chain as a decision of its
/// kind, where the owner sees what the agent did to his playbooks, agents or library.
#[derive(Clone)]
pub struct Author {
    me: SelfCard,
    kind: DecisionKind,
}

impl Capabilities {
    pub fn author_of(me: SelfCard, kind: DecisionKind) -> Author {
        Author { me, kind }
    }
}

impl Author {
    /// The actor a write made through the tool is by: the Chair's card.
    pub(crate) async fn by(&self, o: &Arc<Capabilities>) -> Result<Actor> {
        Ok(by_agent(&self.me.read(o).await?.id))
    }

    /// What a write tool answers: once the write is done, said gives the decision the chain records and the reply; a
    /// write that waits for the owner tells the agent so, and the chain records that he was asked; any other refusal is
    /// the tool's as it is.
    pub(crate) async fn answer<T>(
        &self,
        o: &Arc<Capabilities>,
        written: Result<T>,
        said: impl FnOnce(T) -> (String, String),
    ) -> Result<String> {
        match written {
            Ok(v) => {
                let (decision, reply) = said(v);
                self.record(o, &decision).await?;
                Ok(reply)
            }
            Err(err) => {
                let (Some(words), Error::WaitingForOwner(item)) = (held_words(&err), &err) else {
                    return Err(err);
                };
                self.record(o, &format!("Asked the owner to let through: {}.", item.title))
                    .await?;
                Ok(words)
            }
        }
    }

    /// Records text as a decision of the author's kind on its chain.
    pub(crate) async fn record(&self, o: &Arc<Capabilities>, text: &str) -> Result<()> {
        let me = self.me.read(o).await?;
        o.decide(&me.home, self.kind, text, "", &me.id, by_agent(&me.id)).await;
        Ok(())
    }
}

impl Capabilities {
    /// The tools that read the agent definitions: every Chair's, to hire by them, an author chain's and the
    /// Assistant's.
    pub fn add_agent_read_tools(self: &Arc<Self>, t: &mut Tools) {
        t.serving(self).add(
            "list_agent_definitions", Effect::Reads,
            "Every agent the owner can have hired: id, name, description, group, default model and effort, the tools and skills it holds, the gate it signs, whether it is built in and whether the owner has changed it.",
            |o, _: NoInput| async move {
                #[derive(Serialize)]
                #[serde(rename_all = "PascalCase")]
                struct Row {
                    #[serde(rename = "ID")]
                    id: String,
                    name: String,
                    description: String,
                    group: String,
                    #[serde(skip_serializing_if = "String::is_empty")]
                    model: String,
                    #[serde(skip_serializing_if = "String::is_empty")]
                    effort: String,
                    /// every tool, none, or the list
                    tools: String,
                    #[serde(skip_serializing_if = "Vec::is_empty")]
                    skills: Vec<String>,
                    #[serde(skip_serializing_if = "String::is_empty")]
                    signs: String,
                    builtin: bool,
                    edited: bool,
                }
                let all = o.store.read(|r| r.agents()).await?;
                let rows: Vec<Row> = all
                    .into_iter()
                    .map(|a| Row {
                        tools: tools_label(a.loadout.tools.as_deref()),
                        id: a.id,
                        name: a.name,
                        description: a.description,
                        group: a.group,
                        model: a.model,
                        effort: a.effort,
                        skills: a.loadout.skills,
                        signs: a.signs.to_string(),
                        builtin: a.builtin,
                        edited: a.edited,
                    })
                    .collect();
                as_json(&rows)
            },
        );
        t.serving(self).add(
            "read_agent_definition", Effect::Reads,
            "One agent definition in full, in the shape create_agent_definition and update_agent_definition take. A missing `tools` means it holds every tool; [] means none.",
            |o, i: AgentIdInput| async move {
                let key = i.id.clone();
                let a = o
                    .store
                    .read(move |r| r.agent(&key))
                    .await
                    .map_err(|e| Error::refused(format!("agent {:?}: {e}", i.id)))?;
                as_json(&agent_input(a))
            },
        );
    }

    /// The tools that change the agent definitions; au records each change where the owner sees it.
    pub fn add_agent_write_tools(self: &Arc<Self>, t: &mut Tools, au: &Author) {
        let a = au.clone();
        t.serving(self).add(
            "create_agent_definition", Effect::Writes,
            "Create a new agent. An id already in use is refused. The owner sees it in the agent list at once, and the Chair can hire it by its id.",
            move |o, i: AgentInput| {
                let au = a.clone();
                async move {
                    let by = au.by(&o).await?;
                    let written = match i.agent() {
                        Ok(def) => o.create_agent(&by, def).await,
                        Err(err) => Err(err),
                    };
                    au.answer(&o, written, |a| {
                        (
                            format!("Create agent {}.", a.name),
                            format!("Created agent {} ({}).", a.id, tools_label(a.loadout.tools.as_deref())),
                        )
                    })
                    .await
                }
            },
        );
        let a = au.clone();
        t.serving(self).add(
            "update_agent_definition", Effect::Writes,
            "Replace an existing agent with this version. Every field is replaced, so start from read_agent_definition and keep what the owner did not ask to change. The id stays. Cards already hired keep what they were hired with: the new tools, skills and instructions reach the agents hired from now on.",
            move |o, i: AgentInput| {
                let au = a.clone();
                async move {
                    let by = au.by(&o).await?;
                    let written = match i.agent() {
                        Ok(def) => o.update_agent(&by, def).await,
                        Err(err) => Err(err),
                    };
                    au.answer(&o, written, |a| {
                        (
                            format!("Update agent {}.", a.name),
                            format!("Updated agent {} ({}).", a.id, tools_label(a.loadout.tools.as_deref())),
                        )
                    })
                    .await
                }
            },
        );
        let a = au.clone();
        t.serving(self).add(
            "delete_agent_definition", Effect::Writes,
            "Delete an agent the owner named. A built-in agent cannot be deleted: reset_agent_definition puts it back to its original instead. Cards already hired from it keep running with what they were hired with.",
            move |o, i: AgentIdInput| {
                let au = a.clone();
                async move {
                    let by = au.by(&o).await?;
                    o.delete_agent(&by, &i.id).await?;
                    au.record(&o, &format!("Delete agent {}.", i.id)).await?;
                    Ok(format!("Deleted agent {}.", i.id))
                }
            },
        );
        let a = au.clone();
        t.serving(self).add(
            "reset_agent_definition",
            Effect::Writes,
            "Put a built-in agent back to the version that ships with Remit, discarding the owner's changes to it.",
            move |o, i: AgentIdInput| {
                let au = a.clone();
                async move {
                    let by = au.by(&o).await?;
                    let a = o.reset_agent(&by, &i.id).await?;
                    au.record(&o, &format!("Reset agent {} to its original.", a.name))
                        .await?;
                    Ok(format!("Reset agent {}.", a.id))
                }
            },
        );
    }
}
