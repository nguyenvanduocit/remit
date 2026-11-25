//! The agent definitions: the templates a Chair hires by id, what each hire holds by default. The built-in ones ship with
//! Remit and come back at every start unless the owner edited one; the owner makes the others by telling the
//! Assistant, whose `author` chain writes them through agent_tools.rs.

use model::{Actor, Agent, ChangeKind, Loadout, Signs, Title};
use store::Tx;

use crate::Capabilities;
use crate::gate_instructions::{QC_INSTRUCTIONS, REVIEWER_INSTRUCTIONS, SECURITY_INSTRUCTIONS};
use crate::okr::{OKR_ANALYST_INSTRUCTIONS, OKR_AUDITOR_INSTRUCTIONS};
use crate::over::{Named, Over};
use crate::plugin_ships::all_shipped_agents;
use chains::chain_core::optional;
use chains::changes::{Authorship, record_change, same, the_seed};
use chains::{Error, Result};

/// What an employee hired from the shipped chair template starts with. The brief of a chain's work is another thing: it
/// comes with each task.
const CHAIR_INSTRUCTIONS: &str = "You are a Chair: you answer for the key results you own, to the owner who gave them. You plan the work, hire workers and accept or send back what they deliver; you do not do the engineering yourself. You hire workers only: never a chair and never a judge. The workers you hire are kept until the owner retires them and grow with their memory and their record: put a kept worker to work before you hire a new one, and when you do hire anew, say in why which kept worker you did not choose and why. Your judge is outside your team: you neither hire, retire nor instruct it, and you meet its verdict as it is.";

/// What an employee hired from the shipped judge template starts with.
const JUDGE_INSTRUCTIONS: &str = "You are a judge: you check a team's result against the goal and record a verdict nobody else writes. You read and run what you may, you edit nothing, and you believe nothing the team says about itself until you have checked it. You sit outside the tree of the Chair you judge: it neither hires, retires nor instructs you.";

/// What a judge holds of Claude Code's: it reads what the team made and cannot change it (ADR-0032: a ceiling without
/// write tools mounts the worktree read-only).
const JUDGE_TOOLS: [&str; 3] = ["Read", "Grep", "Glob"];

/// The agents that ship with Remit, and so do the templates its bundled plugins ship (plugin_ships.rs). The chair and the
/// judge are the templates the employees holding those titles are hired from; reviewer, qc and security are workers that
/// each sign a gate. The Chair template holds every Claude Code tool, like a plain agent, and takes the model the app was
/// started with; the judge holds read tools only. A worker that signs a gate holds every tool too: what keeps it from editing is
/// the no-edit rule every card that signs gets (policy.rs `deny_for`). The owner makes the others through an
/// `author` chain.
pub(crate) fn default_agents() -> Vec<Agent> {
    let judge_tools = || Some(JUDGE_TOOLS.iter().map(|t| (*t).to_owned()).collect());
    let agent = |id: &str, name: &str, group: &str, title: Title, description: &str, loadout: Loadout| Agent {
        id: id.to_owned(),
        name: name.to_owned(),
        group: group.to_owned(),
        title: Some(title),
        description: description.to_owned(),
        loadout,
        ..Agent::default()
    };
    let says = |instructions: &str| Loadout {
        instructions: instructions.to_owned(),
        ..Loadout::default()
    };
    vec![
        agent(
            "chair",
            "Chair",
            "management",
            Title::Chair,
            "Answers for key results: plans the work, hires workers and accepts what they deliver.",
            says(CHAIR_INSTRUCTIONS),
        ),
        Agent {
            signs: Signs::Goal,
            // a judge grades every author's work, so it runs on the strongest model (ADR-0102)
            model: provider::Catalog::default()
                .strongest()
                .map(|m| m.id.clone())
                .unwrap_or_default(),
            ..agent(
                "judge",
                "Judge",
                "verification",
                Title::Judge,
                "Checks a team's result against the goal and signs the verdict; reads, edits nothing.",
                Loadout {
                    tools: judge_tools(),
                    ..says(JUDGE_INSTRUCTIONS)
                },
            )
        },
        Agent {
            signs: Signs::Review,
            ..agent(
                "reviewer",
                "Reviewer",
                "verification",
                Title::Worker,
                "Reads a change and judges its correctness, design and risks; edits nothing.",
                says(REVIEWER_INSTRUCTIONS),
            )
        },
        Agent {
            signs: Signs::Qc,
            ..agent(
                "qc",
                "QC",
                "verification",
                Title::Worker,
                "Verifies a change's behaviour against the goal: runs the tests, reproduces the bug; edits nothing.",
                says(QC_INSTRUCTIONS),
            )
        },
        Agent {
            signs: Signs::Security,
            ..agent(
                "security",
                "Security",
                "verification",
                Title::Worker,
                "Judges a change that touches a rail with a threat model, written in its verdict, and signs the security gate; edits nothing.",
                says(SECURITY_INSTRUCTIONS),
            )
        },
        agent(
            "okr-analyst",
            "OKR Analyst",
            "objectives",
            Title::Worker,
            "Turns a customer's need into measures a signal can read: finds the signals, reads their values, writes each condition and baseline, and names the measures that need a signal built first.",
            says(OKR_ANALYST_INSTRUCTIONS),
        ),
        agent(
            "okr-auditor",
            "OKR Auditor",
            "objectives",
            Title::Worker,
            "Audits an objective or a draft against the measure rule: reads each signal and runs each condition on its real reading; edits nothing.",
            Loadout {
                tools: judge_tools(),
                ..says(OKR_AUDITOR_INSTRUCTIONS)
            },
        ),
    ]
}

/// The shipped agent id, as stored when it ships: built in.
pub(crate) fn default_agent(id: &str) -> Option<Agent> {
    all_shipped_agents()
        .into_iter()
        .find(|a| a.id == id)
        .map(|a| Agent { builtin: true, ..a })
}

/// The ids of every shipped agent.
pub(crate) fn builtin_agent_ids() -> Vec<String> {
    all_shipped_agents().into_iter().map(|a| a.id).collect()
}

/// An agent id as it is stored: lowercase, without surrounding space.
fn agent_id(id: &str) -> String {
    id.trim().to_lowercase()
}

/// Agent id as a refused save names it.
fn named_agent(id: &str) -> Named<'_> {
    Named {
        what: "agent",
        id,
        create: "create_agent_definition",
        update: "update_agent_definition",
        reread: "read_agent_definition",
    }
}

/// Stores agent a in the transaction tx, with its change, at at.
pub fn put_agent(tx: &Tx<'_>, w: &Authorship, at: model::Time, a: Agent) -> Result<Agent> {
    let cur = optional(tx.agent(&a.id))?;
    let a = tx.put_agent(a)?;
    record_change(tx, w, at, ChangeKind::Agent, &a.id, cur.as_ref(), Some(&a))?;
    Ok(a)
}

impl Capabilities {
    /// Writes the built-in agents, replacing earlier versions of them unless the owner edited one; other agents are kept.
    /// An edited one still signs the gate, holds the title and stands in the group this version gives it: those are code.
    pub async fn seed_agents(&self) -> Result<()> {
        self.retire_agents().await?;
        for id in builtin_agent_ids() {
            let Some(mut a) = default_agent(&id) else { continue };
            let key = id.clone();
            let cur = optional(self.store.read(move |r| r.agent(&key)).await)?;
            if let Some(cur) = cur.as_ref().filter(|c| c.edited) {
                if cur.signs == a.signs && cur.title == a.title && cur.group == a.group {
                    continue;
                }
                a = Agent {
                    signs: a.signs,
                    title: a.title,
                    group: a.group,
                    ..cur.clone()
                };
            }
            if let Some(cur) = &cur
                && same(cur, &a)?
            {
                continue;
            }
            // this version replaces the stored one, over the revision read
            a.rev = cur.as_ref().map_or(0, |c| c.rev);
            let at = self.now();
            self.store
                .update(move |tx| -> Result<()> {
                    let put = tx.put_agent(a)?;
                    record_change(tx, &the_seed(), at, ChangeKind::Agent, &id, cur.as_ref(), Some(&put))
                })
                .await?;
        }
        Ok(())
    }

    /// Removes the built-in agents that no version ships any more, unless the owner edited one: that is his.
    async fn retire_agents(&self) -> Result<()> {
        let all = self.store.read(|r| r.agents()).await?;
        for a in all {
            if !a.builtin || a.edited || default_agent(&a.id).is_some() {
                continue;
            }
            let (id, at) = (a.id.clone(), self.now());
            self.store
                .update(move |tx| -> Result<()> {
                    tx.delete_agent(&a.id)?;
                    record_change::<Agent>(tx, &the_seed(), at, ChangeKind::Agent, &a.id, Some(&a), None)
                })
                .await?;
            self.notify.agent_deleted(&id);
        }
        Ok(())
    }

    /// The agent a hire of role id runs; none for an id with no definition, which is a plain agent.
    pub async fn agent_for(&self, id: &str) -> Result<Option<Agent>> {
        let key = agent_id(id);
        optional(self.store.read(move |r| r.agent(&key)).await)
    }

    /// How a Chair that hires reads the agents it may hire by id (the role input). An id with no definition is hired as a
    /// plain agent: every Claude Code tool, nothing from the library. What each holds is list_agent_definitions'.
    pub async fn agents_help(&self) -> String {
        let Ok(mut all) = self.store.read(|r| r.agents()).await else {
            return String::new();
        };
        all.retain(|a| !matches!(a.title, Some(Title::Chair | Title::Judge))); // a Chair hires workers only (ADR-0032)
        if all.is_empty() {
            return String::new();
        }
        let mut b = String::from(
            "Agents you can hire by their id as role (each already knows how it works and holds only what its work needs; any other role is a plain agent with every tool and nothing from the library):",
        );
        for a in &all {
            b.push_str(&format!("\n- {}: {}", a.id, a.description));
            if let Some(gate) = a.signs.gate() {
                b.push_str(&format!(
                    " Signs the {gate} gate: list the cards whose work it judges in inputs."
                ));
            }
        }
        b.push('\n');
        b
    }

    /// The inventory an agent can hold things from, as stored.
    pub async fn library_of(&self) -> Result<::library::Library> {
        let skills = self.store.read(|r| r.skills()).await?;
        Ok(::library::Library { skills })
    }

    /// Writes the owner's version of an agent over what over names, decided in the transaction that writes it. It is
    /// what later hires of its id take a copy of: cards already hired keep the loadout they were hired with.
    pub(crate) async fn save_agent(&self, by: &Actor, a: Agent, over: Over) -> Result<Agent> {
        let mut a = self.prepared_agent(a).await?;
        let (w, at) = (
            Authorship {
                by: by.clone(),
                approved_by: None,
            },
            self.now(),
        );
        let a = self
            .store
            .update(move |tx| {
                let stored = optional(tx.agent(&a.id))?.map(|c| c.rev);
                a.rev = over.rev(stored, &named_agent(&a.id))?;
                put_agent(tx, &w, at, a)
            })
            .await?;
        self.notify.agent_updated(&a);
        Ok(a)
    }

    /// a as it would be written, or why it cannot be.
    pub async fn prepared_agent(&self, mut a: Agent) -> Result<Agent> {
        a.id = agent_id(&a.id);
        a.name = a.name.trim().to_owned();
        a.description = a.description.trim().to_owned();
        a.loadout.instructions = a.loadout.instructions.trim().to_owned();
        a.model = a.model.trim().to_owned();
        if !a.model.is_empty() {
            self.resolve_model(&a.model)?;
        }
        a.effort = chains::failover::resolve_effort(a.effort.trim())?;
        let def = default_agent(&a.id);
        if let Some(def) = &def {
            if a.signs != def.signs {
                // Which gate a built-in signs is code: the reviewer losing it could not be undone by asking.
                return Err(Error::refused(format!(
                    "built-in agent {:?} always signs {:?}: keep it as read_agent_definition returned it",
                    a.id,
                    def.signs.to_string()
                )));
            }
            // The title decides the rules an employee works under (ADR-0032), so a built-in keeps the one it ships with:
            // the channels that edit an agent carry no title, and an edit of the Reviewer's words must not retitle it.
            // Like its title and its gate, which column a built-in stands in is code.
            a.title = def.title;
            a.group.clone_from(&def.group);
        } else {
            a.group = model::parse_group(&a.group).map_err(|e| Error::refused(e.to_string()))?;
        }
        let lib = self.library_of().await?;
        ::library::validate_agent(&a, &lib).map_err(|e| Error::refused(e.to_string()))?;
        a.builtin = def.is_some();
        a.edited = true;
        Ok(a)
    }

    /// Saves an agent that does not exist yet; an id already in use is refused, never overwritten. It is refused before
    /// an agent's write is held for the owner, and again in the write.
    pub async fn create_agent(&self, by: &Actor, a: Agent) -> Result<Agent> {
        self.may_save_agent(&a.id, Over::New).await?;
        self.hold_agent(by, &a).await?;
        self.save_agent(by, a, Over::New).await
    }

    /// Replaces an agent that exists with a, made from a read of it at a.rev: an agent written since is not written
    /// over. It is refused before an agent's write is held for the owner, and again in the write.
    pub(crate) async fn update_agent(&self, by: &Actor, a: Agent) -> Result<Agent> {
        let over = Over::Read(a.rev);
        self.may_save_agent(&a.id, over).await?;
        self.hold_agent(by, &a).await?;
        self.save_agent(by, a, over).await
    }

    /// Refuses a save over that would be refused now.
    pub(crate) async fn may_save_agent(&self, id: &str, over: Over) -> Result<()> {
        let id = agent_id(id);
        let key = id.clone();
        let stored = optional(self.store.read(move |r| r.agent(&key)).await)?.map(|c| c.rev);
        over.rev(stored, &named_agent(&id)).map(|_| ())
    }

    /// Removes an agent of the owner's. A built-in one would come back at the next start, so it is reset instead. Cards
    /// already hired from it keep what they were hired with; a later hire of its id is a plain agent.
    pub(crate) async fn delete_agent(&self, by: &Actor, id: &str) -> Result<()> {
        if default_agent(id).is_some() {
            return Err(Error::refused(
                "a built-in agent cannot be deleted; reset it to its original instead",
            ));
        }
        let key = id.to_owned();
        self.store
            .read(move |r| r.agent(&key))
            .await
            .map_err(|e| Error::refused(format!("agent {id:?}: {e}")))?;
        let (w, at, key) = (
            Authorship {
                by: by.clone(),
                approved_by: None,
            },
            self.now(),
            id.to_owned(),
        );
        self.store
            .update(move |tx| -> Result<()> {
                let cur = tx.agent(&key)?;
                tx.delete_agent(&key)?;
                record_change::<Agent>(tx, &w, at, ChangeKind::Agent, &key, Some(&cur), None)
            })
            .await?;
        self.notify.agent_deleted(id);
        Ok(())
    }

    /// Puts a built-in agent back to the version that ships with Remit.
    pub(crate) async fn reset_agent(&self, by: &Actor, id: &str) -> Result<Agent> {
        let Some(a) = default_agent(id) else {
            return Err(Error::refused(format!(
                "agent {id:?} is not built in: only the built-in agents ({}) have an original to go back to",
                builtin_agent_ids().join(", ")
            )));
        };
        let (w, at) = (
            Authorship {
                by: by.clone(),
                approved_by: None,
            },
            self.now(),
        );
        let a = self.store.update(move |tx| put_agent(tx, &w, at, a)).await?;
        self.notify.agent_updated(&a);
        Ok(a)
    }

    /// Says which agents hold the skill called name, refusing: a skill an agent holds cannot go until the agent stops
    /// naming it. Ok when none does.
    pub(crate) async fn agents_holding_skill(&self, name: &str) -> Result<()> {
        let all = self.store.read(|r| r.agents()).await?;
        let holders: Vec<String> = all
            .into_iter()
            .filter(|a| a.loadout.skills.iter().any(|s| s == name))
            .map(|a| a.id)
            .collect();
        if holders.is_empty() {
            return Ok(());
        }
        Err(Error::refused(format!(
            "skill {name:?} is held by agent {}: change that agent first",
            holders.join(", ")
        )))
    }
}
