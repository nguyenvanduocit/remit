//! The Assistant's project tools (ADR-0108): how the owner keeps his products by telling it. Each write is the action in
//! project.rs, taken as the Assistant for him, so he sees it at once on the Projects tab with who made it, and its history
//! (list_changes, kind project). `set_project` moves any chain onto a project through the one path that sets a chain's
//! project (`chains::project::assign`); the Chair's own, for its chain, is in orch's card_tools.rs.

use std::sync::Arc;

use model::{Actor, ConnectionRef, Project, ProjectStatus};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Crew;
use crate::project::{PROJECT_BRIEF_MAX, ProjectInput};
use chains::chain_core::{first_line, optional};
use chains::project::{Assignment, named_project};
use chains::tools::{Effect, NoInput, Tools, as_json};
use chains::{Error, Result};

/// A connection a project lists, as list_sources names it.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ConnectionInput {
    /// the source's id, as list_sources shows it, e.g. datadog
    source: String,
    /// the connection's name on that source, e.g. prod-a
    connection: String,
}

impl From<ConnectionInput> for ConnectionRef {
    fn from(c: ConnectionInput) -> Self {
        ConnectionRef {
            source: c.source,
            connection: c.connection,
        }
    }
}

/// A project as create_project takes it.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CreateProjectInput {
    /// the name the owner calls the product by, unique among projects, e.g. checkout
    name: String,
    /// who the product is for and what they get, in his words: where each Chair's CUSTOMER section starts
    #[serde(default)]
    customer: String,
    /// his notes on the product, in Markdown: what it is, where it runs, what matters. Every Chair of the project reads
    /// them; what a repository says of itself (its AGENTS.md, its checks) stays in the repository
    #[serde(default)]
    brief: String,
    /// its repositories, each by its remote: host/group/project (github.com/owner/name), a remote URL, or the path of a
    /// checkout on this machine (/Users/me/code/shop, ~/code/shop), stored as its origin; they need not be cloned yet
    #[serde(default)]
    repos: Vec<String>,
    /// the connections of sources that are this product's, each {source, connection} as list_sources shows them
    #[serde(default)]
    connections: Vec<ConnectionInput>,
}

impl From<CreateProjectInput> for ProjectInput {
    fn from(i: CreateProjectInput) -> Self {
        ProjectInput {
            name: i.name,
            customer: i.customer,
            brief: i.brief,
            repos: i.repos,
            connections: i.connections.into_iter().map(ConnectionRef::from).collect(),
        }
    }
}

/// A project whole as update_project writes it, over the revision read_project showed.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct UpdateProjectInput {
    /// the project's id (p_…), as list_projects shows it
    id: String,
    /// its revision as read_project showed it: a project changed since is refused, so no change is lost
    rev: i64,
    /// its name, unique among projects
    name: String,
    /// who the product is for and what they get
    #[serde(default)]
    customer: String,
    /// the owner's notes on the product, in Markdown
    #[serde(default)]
    brief: String,
    /// its repositories, each by its remote
    #[serde(default)]
    repos: Vec<String>,
    /// its connections, each {source, connection}
    #[serde(default)]
    connections: Vec<ConnectionInput>,
}

impl UpdateProjectInput {
    fn fields(self) -> (String, i64, ProjectInput) {
        let input = ProjectInput {
            name: self.name,
            customer: self.customer,
            brief: self.brief,
            repos: self.repos,
            connections: self.connections.into_iter().map(ConnectionRef::from).collect(),
        };
        (self.id, self.rev, input)
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProjectIdInput {
    /// the project's id (p_…), as list_projects shows it
    id: String,
}

/// A chain moved onto a project, or off every one.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SetProjectInput {
    /// the chain's id (ch_…)
    chain_id: String,
    /// the id of an active project (p_…, list_projects); empty: the chain is about no product
    #[serde(default)]
    project: String,
    /// one line the owner reads in the chain's history: why this project, in his words
    why: String,
}

/// A project as list_projects shows it: what tells one from another, its notes cut to their first line.
#[derive(Serialize)]
struct ProjectRow {
    id: String,
    rev: i64,
    name: String,
    status: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    customer: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    about: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    repos: Vec<String>,
}

fn project_row(p: Project) -> ProjectRow {
    ProjectRow {
        about: first_line(&p.brief, 160),
        status: p.status.as_str().to_owned(),
        id: p.id,
        rev: p.rev,
        name: p.name,
        customer: p.customer,
        repos: p.repos,
    }
}

/// What every tool that writes a project reminds the Assistant of: a project is context, so writing one widens nothing.
const PROJECT_IS_CONTEXT: &str = "A project is context its Chairs read: it grants nothing, and a connection it lists still needs the chain's capability or a grant to be read.";

impl Crew {
    /// Gives the Assistant, acting as by, the tools that keep the owner's projects and set a chain's.
    pub fn add_project_assistant_tools(self: &Arc<Self>, t: &mut Tools, by: &Actor) {
        self.add_project_read_tools(t);
        self.add_project_write_tools(t, by);
    }

    /// Gives list_projects and read_project: the Assistant reads what it keeps, and a Chair that drafts an objective reads
    /// which product it is about.
    pub fn add_project_read_tools(self: &Arc<Self>, t: &mut Tools) {
        t.serving(self).add(
            "list_projects", Effect::Reads,
            "The owner's projects, one per product his work is on, archived ones too: each with its id, revision, name, status (active, or archived: offered to no one), customer, the first line of his notes and its repositories. read_project gives one whole.",
            |o, _: NoInput| async move {
                let all = o.store.read(|r| r.projects()).await?;
                as_json(&all.into_iter().map(project_row).collect::<Vec<_>>())
            },
        );
        t.serving(self).add(
            "read_project", Effect::Reads,
            "One project whole, as update_project takes it: its revision, name, customer, the owner's notes, repositories and connections, who last changed it and when. Its history is list_changes with kind project and its id.",
            |o, i: ProjectIdInput| async move {
                let id = i.id.clone();
                match optional(o.store.read(move |r| r.project(&id)).await)? {
                    Some(p) => as_json(&p),
                    None => Err(Error::refused(format!("no project {:?}: list_projects shows the ids", i.id))),
                }
            },
        );
    }

    /// Gives the Assistant, acting as by, the tools that write a project and set a chain's.
    fn add_project_write_tools(self: &Arc<Self>, t: &mut Tools, by: &Actor) {
        let b = by.clone();
        t.serving(self).add(
            "create_project", Effect::Writes,
            format!(
                "Make a project on the owner's word: one product his work is on, with who it is for, his notes on it (at most {PROJECT_BRIEF_MAX} characters), its repositories and the connections that are its own. Check list_projects first, so a product is not made twice. Chains about it then start from it: their Chair reads it in its brief. {PROJECT_IS_CONTEXT}"
            ),
            move |o, i: CreateProjectInput| {
                let by = b.clone();
                async move {
                    let p = o.create_project(i.into(), by).await?;
                    Ok(format!("Made project {:?} ({}).", p.name, p.id))
                }
            },
        );
        let b = by.clone();
        t.serving(self).add(
            "update_project", Effect::Writes,
            format!(
                "Replace a project's fields with what the owner asked: read_project first and write it whole, changing what he said, so the rest stays; give the revision you read. A Chair reads the new version when its session next starts. {PROJECT_IS_CONTEXT}"
            ),
            move |o, i: UpdateProjectInput| {
                let by = b.clone();
                async move {
                    let (id, rev, input) = i.fields();
                    let p = o.update_project(&id, rev, input, by).await?;
                    Ok(format!("Project {:?} ({}) is at revision {}.", p.name, p.id, p.rev))
                }
            },
        );
        let b = by.clone();
        t.serving(self).add(
            "archive_project", Effect::Writes,
            "Archive a project when the owner is done with the product: it is offered to no one from now on (no Chair picks it), and the objectives and chains that name it keep it. Nothing is deleted.",
            move |o, i: ProjectIdInput| {
                let by = b.clone();
                async move {
                    let p = o.set_project_status(&i.id, ProjectStatus::Archived, by).await?;
                    Ok(format!("Project {:?} ({}) is archived.", p.name, p.id))
                }
            },
        );
        let b = by.clone();
        t.serving(self).add(
            "restore_project", Effect::Writes,
            "Bring an archived project back when the owner asks: it is offered again to the Chairs that choose a chain's project.",
            move |o, i: ProjectIdInput| {
                let by = b.clone();
                async move {
                    let p = o.set_project_status(&i.id, ProjectStatus::Active, by).await?;
                    Ok(format!("Project {:?} ({}) is active.", p.name, p.id))
                }
            },
        );
        let b = by.clone();
        t.serving(self).add(
            "set_project", Effect::Writes,
            "Move a chain onto the project the owner says it is about, or off every project (project empty). It is recorded in the chain's history as yours, with his why; a Chair at work reads the project when its session next starts, so tell it with message_agent when it should work from it now. A chain serving an objective he applied takes that objective's project: this is refused, and the objective is amended instead.",
            move |o, i: SetProjectInput| {
                let by = b.clone();
                async move { o.set_project(by, i).await }
            },
        );
    }

    /// Sets the project of chain i.chain_id as the Assistant, by, for the owner.
    async fn set_project(&self, by: Actor, i: SetProjectInput) -> Result<String> {
        let why = i.why.trim().to_owned();
        if why.is_empty() {
            return Err(Error::refused("say why: the owner reads it in the chain's history"));
        }
        let project = named_project(&i.project);
        let a = Assignment {
            project: project.clone(),
            by,
            why,
            card_id: String::new(),
        };
        self.assign(&i.chain_id, a).await?;
        Ok(match project {
            Some(id) => format!("Chain {} is on project {id}.", i.chain_id),
            None => format!("Chain {} is on no project.", i.chain_id),
        })
    }
}
