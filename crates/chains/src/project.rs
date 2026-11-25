//! A chain's project (ADR-0108): the product its work is on. A chain's project is its served objective's, read by
//! `project_of` and never stored on the chain; `assign_in` is the one path that sets it, whoever chooses (the owner or the
//! Assistant naming one, `set_project`). A project is context: it grants nothing, and nothing here branches
//! on which one it is.

use model::{Actor, Chain, Decision, DecisionKind, Objective, Project, ProjectStatus, Time};

use crate::chain_core::{Core, new_id};
use crate::{Error, Result};

/// A choice of a chain's project: which one (none: the chain is about no product), who chose it, why, and the card that
/// chose it (empty: none).
#[derive(Debug, Clone, Default)]
pub struct Assignment {
    pub project: Option<String>,
    pub by: Actor,
    pub why: String,
    pub card_id: String,
}

/// The project an agent's input names by its id: none when it names none (empty). Pure.
pub fn named_project(id: &str) -> Option<String> {
    Some(id.trim().to_owned()).filter(|p| !p.is_empty())
}

/// What a Chair is told of projects: its chain's, or the active ones it chooses from in its first turn. With none to
/// choose from (none defined, or its objective is one the owner applied, which carries its own) it is told nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectBrief {
    Its(Box<Project>),
    Choose(Vec<Project>),
}

/// The objective chain ch serves; none for a chain stored before every chain served one.
fn served(r: &store::Reader<'_>, ch: &Chain) -> store::Result<Option<Objective>> {
    ch.serves.as_ref().map(|s| r.objective(&s.objective)).transpose()
}

/// The id of chain ch's project: its served objective's, none when that names none.
pub fn project_of(r: &store::Reader<'_>, ch: &Chain) -> store::Result<Option<String>> {
    Ok(served(r, ch)?.and_then(|o| o.project))
}

/// What chain ch's Chair is told of projects (`ProjectBrief`).
pub fn brief_of(r: &store::Reader<'_>, ch: &Chain) -> store::Result<ProjectBrief> {
    let Some(o) = served(r, ch)? else {
        return Ok(ProjectBrief::Choose(Vec::new()));
    };
    if let Some(id) = &o.project {
        return Ok(ProjectBrief::Its(Box::new(r.project(id)?)));
    }
    if !o.task {
        return Ok(ProjectBrief::Choose(Vec::new()));
    }
    Ok(ProjectBrief::Choose(active(r.projects()?)))
}

/// The projects offered to the agents that choose a chain's project: the active ones, in the order given.
pub fn active(projects: Vec<Project>) -> Vec<Project> {
    projects
        .into_iter()
        .filter(|p| p.status == ProjectStatus::Active)
        .collect()
}

/// Objective o with project p (none: no project), or why it may not take it: an objective the owner applied carries its
/// own project, moved by amending the objective, and an archived project is offered to no one. None when o has it
/// already. Pure.
pub fn assigned(o: &Objective, p: Option<&Project>) -> Result<Option<Objective>> {
    let id = p.map(|p| p.id.as_str());
    if o.project.as_deref() == id {
        return Ok(None);
    }
    if !o.task {
        return Err(Error::refused(format!(
            "the chain serves objective {}, which the owner applied: its project is the objective's, so amend the objective",
            o.id
        )));
    }
    if let Some(p) = p
        && p.status != ProjectStatus::Active
    {
        return Err(Error::refused(format!("project {:?} ({}) is archived", p.name, p.id)));
    }
    let mut o = o.clone();
    o.project = id.map(str::to_owned);
    Ok(Some(o))
}

/// The decision the owner reads of a, which set chain chain_id's project to p, at now. Pure.
fn decision_of(chain_id: &str, p: Option<&Project>, a: &Assignment, now: Time) -> Decision {
    Decision {
        id: new_id("d_"),
        chain_id: chain_id.to_owned(),
        kind: DecisionKind::Project,
        by: Some(a.by.clone()),
        text: p.map_or_else(|| "No project.".to_owned(), |p| format!("Project: {}.", p.name)),
        why: a.why.clone(),
        card_id: a.card_id.clone(),
        created_at: now,
        ..Decision::default()
    }
}

/// Sets chain ch's project as a says, in tx, at now: the one path that does (ADR-0108). Its served objective is written
/// over the revision read, with a `project` decision; nothing is written when the objective has the project already.
/// The decision stored, for the caller to tell once tx commits.
pub fn assign_in(tx: &store::Tx<'_>, ch: &Chain, a: &Assignment, now: Time) -> Result<Option<Decision>> {
    let Some(o) = served(tx, ch)? else {
        return Err(Error::refused(format!(
            "chain {} serves no objective to hold its project",
            ch.id
        )));
    };
    let p = match &a.project {
        Some(id) => Some(tx.project(id).map_err(|e| match e {
            store::Error::NotFound => Error::refused(format!("no project {id}")),
            e => Error::Store(e),
        })?),
        None => None,
    };
    let Some(o) = assigned(&o, p.as_ref())? else {
        return Ok(None);
    };
    tx.put_objective(o)?;
    let d = decision_of(&ch.id, p.as_ref(), a, now);
    tx.put_decision(&d)?;
    Ok(Some(d))
}

impl Core {
    /// Sets chain chain_id's project as a says (`assign_in`), and tells the channels of the decision and of the chain's
    /// views, whose standing names its project.
    pub async fn assign(&self, chain_id: &str, a: Assignment) -> Result<()> {
        let ch = self.chain(chain_id).await?;
        let now = self.now();
        let decided = self.store.update(move |tx| assign_in(tx, &ch, &a, now)).await?;
        if let Some(d) = decided {
            self.told_assigned(&d);
        }
        Ok(())
    }

    /// Tells the channels of d, a project decision stored: the decision, and its chain's views.
    pub fn told_assigned(&self, d: &Decision) {
        self.notify.decision_added(d);
        self.notify.views_moved(&d.chain_id);
    }

    /// What chain ch's Chair is told of projects.
    pub async fn project_brief(&self, ch: &Chain) -> Result<ProjectBrief> {
        let ch = ch.clone();
        Ok(self.store.read(move |r| brief_of(r, &ch)).await?)
    }
}
