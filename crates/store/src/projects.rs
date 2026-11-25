//! Projects: the products the owner's work is on (ADR-0108). One document with a revision, like an objective.

use model::Project;
use rusqlite::params;

use crate::store::{Error, Reader, Result, Tx, doc, ms, over_read_rev, unique_failed};

impl Tx<'_> {
    /// Writes p over the revision it was read at, p.rev (0 for a new project), and returns it at the revision stored.
    /// `Error::ProjectName` when another project has its name, in any case; `Error::Conflict` when the stored project is
    /// at another revision, or a new one's id is taken.
    pub fn put_project(&self, mut p: Project) -> Result<Project> {
        p.rev = p.rev.saturating_add(1);
        let rev = self.put_doc(
            concat!(
                "INSERT INTO projects(id, name, created_at, doc) VALUES(?,?,?,?)
ON CONFLICT(id) DO UPDATE SET name=excluded.name, doc=excluded.doc",
                over_read_rev!("projects"),
                "
RETURNING json_extract(doc, '$.rev')"
            ),
            params![p.id, p.name, ms(p.created_at), doc(&p)?],
        );
        match rev {
            Err(Error::Sqlite(e)) if unique_failed(&e, "projects.name") => Err(Error::ProjectName),
            Err(e) => Err(e),
            Ok(rev) => {
                p.rev = rev;
                Ok(p)
            }
        }
    }
}

impl Reader<'_> {
    pub fn project(&self, id: &str) -> Result<Project> {
        self.one("SELECT doc FROM projects WHERE id=?", [id])
    }

    /// Every project, archived ones too, by name.
    pub fn projects(&self) -> Result<Vec<Project>> {
        self.list("SELECT doc FROM projects ORDER BY name COLLATE NOCASE, id", [])
    }
}
