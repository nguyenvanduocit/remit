//! The library: the agents the owner defined, and the skills they can hold. Each is one document with a revision, in the
//! shape of a playbook.

use model::{Agent, Skill};
use rusqlite::params;

use crate::store::{Reader, Result, Tx, doc, over_read_rev};

impl Tx<'_> {
    /// Writes a over the revision it was read at, a.rev (0 for a new agent), and returns it at the revision stored.
    /// `Error::Conflict` when the stored agent is at another revision, or a new one's id is taken.
    pub fn put_agent(&self, mut a: Agent) -> Result<Agent> {
        a.rev = a.rev.saturating_add(1);
        a.rev = self.put_doc(
            concat!(
                "INSERT INTO agents(id, doc) VALUES(?,?) ON CONFLICT(id) DO UPDATE SET doc=excluded.doc",
                over_read_rev!("agents"),
                " RETURNING json_extract(doc, '$.rev')"
            ),
            params![a.id, doc(&a)?],
        )?;
        Ok(a)
    }

    pub fn delete_agent(&self, id: &str) -> Result<()> {
        self.exec("DELETE FROM agents WHERE id=?", [id])?;
        Ok(())
    }

    /// Writes s over the revision it was read at, s.rev (0 for a new skill), and returns it at the revision stored.
    /// `Error::Conflict` when the stored skill is at another revision, or a new one's name is taken.
    pub fn put_skill(&self, mut s: Skill) -> Result<Skill> {
        s.rev = s.rev.saturating_add(1);
        s.rev = self.put_doc(
            concat!(
                "INSERT INTO skills(name, doc) VALUES(?,?) ON CONFLICT(name) DO UPDATE SET doc=excluded.doc",
                over_read_rev!("skills"),
                " RETURNING json_extract(doc, '$.rev')"
            ),
            params![s.name, doc(&s)?],
        )?;
        Ok(s)
    }

    pub fn delete_skill(&self, name: &str) -> Result<()> {
        self.exec("DELETE FROM skills WHERE name=?", [name])?;
        Ok(())
    }
}

impl Reader<'_> {
    pub fn agent(&self, id: &str) -> Result<Agent> {
        self.one("SELECT doc FROM agents WHERE id=?", [id])
    }

    pub fn agents(&self) -> Result<Vec<Agent>> {
        self.list("SELECT doc FROM agents ORDER BY id", [])
    }

    pub fn skill(&self, name: &str) -> Result<Skill> {
        self.one("SELECT doc FROM skills WHERE name=?", [name])
    }

    pub fn skills(&self) -> Result<Vec<Skill>> {
        self.list("SELECT doc FROM skills ORDER BY name", [])
    }
}
