//! The projects the owner keeps (ADR-0108): one product each, the durable context its objectives and their chains carry.
//! The Assistant writes them on his words (project_tools.rs): each write is over the revision it read, stores a `Change` of
//! kind `project` in its transaction (who, before, after) and is pushed as `project_updated`. Nothing waits for an Apply,
//! since a project moves no rail: it grants nothing, so a connection it lists is checked to exist, never to permit.

use model::{Actor, ChangeKind, ConnectionRef, Project, ProjectStatus};

use crate::Crew;
use chains::chain_core::{new_id, optional};
use chains::changes::{Authorship, record_change};
use chains::git::{folder_named, parse_remote, path_str};
use chains::{Error, Result};
use reach::grants::{connection_in, connection_names};

/// The longest the owner's notes on a project may be, in characters: every Chair of the project reads them in its brief.
pub const PROJECT_BRIEF_MAX: usize = 4000;

/// A project's fields as the owner describes it, whole: what create_project takes and update_project replaces.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectInput {
    pub name: String,
    pub customer: String,
    pub brief: String,
    /// Its repository remotes or local folder paths.
    pub repos: Vec<String>,
    pub connections: Vec<ConnectionRef>,
}

impl ProjectInput {
    /// Validates the input, normalizes repository names and drops repeated repositories and connections.
    pub fn checked(&self) -> Result<Self> {
        let name = self.name.trim().to_owned();
        if name.is_empty() {
            return Err(Error::refused(
                "a project has a name: the one the owner calls the product by",
            ));
        }
        let brief = self.brief.trim().to_owned();
        let length = brief.chars().count();
        if length > PROJECT_BRIEF_MAX {
            return Err(Error::refused(format!(
                "the notes are {length} characters, past the {PROJECT_BRIEF_MAX} every Chair of the project reads: keep \
                 what matters, and leave what a repository says of itself (its AGENTS.md, its checks) in the repository"
            )));
        }
        let mut repos: Vec<String> = Vec::with_capacity(self.repos.len());
        for r in &self.repos {
            let named = if let Some(dir) = folder_named(r) {
                path_str(&dir)
            } else if let Some((remote, _)) = parse_remote(r) {
                remote.named()
            } else {
                return Err(Error::refused(format!(
                    "{r:?} is no repository name: use host/group/project (github.com/owner/name), a remote URL or \
                     an absolute local folder path"
                )));
            };
            if !repos.contains(&named) {
                repos.push(named);
            }
        }
        let mut connections: Vec<ConnectionRef> = Vec::with_capacity(self.connections.len());
        for c in &self.connections {
            let c = ConnectionRef {
                source: c.source.trim().to_owned(),
                connection: c.connection.trim().to_owned(),
            };
            if c.source.is_empty() || c.connection.is_empty() {
                return Err(Error::refused(
                    "a connection is named by its source and its name, as list_sources shows them",
                ));
            }
            if !connections.contains(&c) {
                connections.push(c);
            }
        }
        Ok(Self {
            name,
            customer: self.customer.trim().to_owned(),
            brief,
            repos,
            connections,
        })
    }

    /// p with this input's fields. Pure.
    fn applied_to(self, p: Project) -> Project {
        Project {
            name: self.name,
            customer: self.customer,
            brief: self.brief,
            repos: self.repos,
            connections: self.connections,
            ..p
        }
    }
}

impl Crew {
    /// Makes a project from the owner's words, as by, and tells the channels.
    pub async fn create_project(&self, input: ProjectInput, by: Actor) -> Result<Project> {
        let input = input.checked()?;
        self.connections_exist(&input.connections).await?;
        let now = self.now();
        let fresh = input.applied_to(Project {
            id: new_id("p_"),
            status: ProjectStatus::Active,
            created_at: now,
            ..Project::default()
        });
        let id = fresh.id.clone();
        self.write_project(&id, by, move |_| Ok(Some(fresh))).await
    }

    /// Replaces project id's fields with input, over revision rev, the one the writer read: a project changed since is
    /// refused, so no one's change is lost. Its status stays.
    pub async fn update_project(&self, id: &str, rev: i64, input: ProjectInput, by: Actor) -> Result<Project> {
        let input = input.checked()?;
        self.connections_exist(&input.connections).await?;
        let key = id.to_owned();
        self.write_project(id, by, move |cur| {
            let Some(cur) = cur else {
                return Err(no_project(&key));
            };
            if cur.rev != rev {
                return Err(Error::refused(format!(
                    "project {key} changed since you read it (it is at revision {}, you read {rev}): read_project \
                         again and write it whole",
                    cur.rev
                )));
            }
            let next = input.applied_to(cur.clone());
            Ok((next != *cur).then_some(next))
        })
        .await
    }

    /// Sets project id's status: archived, it is offered to no one and stays read on the objectives that name it; active,
    /// it is offered again. A project with that status already stays as it is.
    pub async fn set_project_status(&self, id: &str, status: ProjectStatus, by: Actor) -> Result<Project> {
        let key = id.to_owned();
        self.write_project(id, by, move |cur| {
            let Some(cur) = cur else {
                return Err(no_project(&key));
            };
            Ok((cur.status != status).then(|| Project { status, ..cur.clone() }))
        })
        .await
    }

    /// Stores what change makes of project id as stored (none for a new one), read and written in one transaction with its
    /// `Change`, as by, and tells the channels. change returns none when the project already says it: nothing is written,
    /// and the project is returned as stored.
    async fn write_project<F>(&self, id: &str, by: Actor, change: F) -> Result<Project>
    where
        F: FnOnce(Option<&Project>) -> Result<Option<Project>> + Send + 'static,
    {
        let (key, now) = (id.to_owned(), self.now());
        let w = Authorship {
            by: by.clone(),
            approved_by: None,
        };
        let written = self
            .store
            .update(move |tx| -> Result<(Project, bool)> {
                let cur = optional(tx.project(&key))?;
                let Some(mut next) = change(cur.as_ref())? else {
                    return Ok((cur.ok_or_else(|| no_project(&key))?, false));
                };
                (next.by, next.updated_at) = (by, now);
                record_change(tx, &w, now, ChangeKind::Project, &key, cur.as_ref(), Some(&next))?;
                let name = next.name.clone();
                match tx.put_project(next) {
                    Err(store::Error::ProjectName) => Err(Error::refused(format!(
                        "a project named {name:?} exists already: list_projects shows it"
                    ))),
                    Err(store::Error::Conflict) => Err(Error::refused(format!(
                        "project {key} changed while this was written: nothing was written; read_project and write it again"
                    ))),
                    stored => Ok((stored?, true)),
                }
            })
            .await?;
        let (p, wrote) = written;
        if wrote {
            self.notify.project_updated(&p);
        }
        Ok(p)
    }

    /// Refuses a connection that does not exist: a source Remit does not know, or a name its source has no connection
    /// of. It checks existence only: a project grants nothing.
    async fn connections_exist(&self, conns: &[ConnectionRef]) -> Result<()> {
        for c in conns {
            if self.sources.blank(&c.source).is_none() {
                return Err(Error::refused(format!(
                    "no source {:?}: the ones that exist are {}",
                    c.source,
                    self.sources.source_ids().join(", ")
                )));
            }
            let source = c.source.clone();
            let stored = self.store.read(move |r| r.source_connections(&source)).await?;
            if !connection_in(&stored, &c.connection) {
                return Err(Error::refused(format!(
                    "{} has no connection {:?}: the ones that exist are {}",
                    c.source,
                    c.connection,
                    connection_names(&stored)
                )));
            }
        }
        Ok(())
    }
}

fn no_project(id: &str) -> Error {
    Error::refused(format!("no project {id:?}: list_projects shows the ids"))
}

#[cfg(test)]
mod tests {
    use super::ProjectInput;

    #[test]
    fn a_project_keeps_a_local_path_alongside_its_remotes() {
        let input = ProjectInput {
            name: "Storefront".into(),
            repos: vec![
                "/work/Storefront experiments".into(),
                "https://github.com/owner/storefront.git".into(),
                "github.com/owner/storefront".into(),
            ],
            ..ProjectInput::default()
        };

        let checked = input.checked().unwrap();
        assert_eq!(
            checked.repos,
            ["/work/Storefront experiments", "github.com/owner/storefront"]
        );
        assert_eq!(checked.checked().unwrap(), checked);
    }
}
