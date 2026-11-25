//! Mentors and helps (ADR-0062): a mentor is a person who helps the AI employees; a help is one mentor on one card. Each
//! is one document with a revision, like an employee.

use model::{Help, Mentor};
use rusqlite::params;

use crate::store::{Reader, Result, Tx, doc, ms, over_read_rev};

impl Tx<'_> {
    /// Writes m over the revision it was read at, m.rev (0 for a new mentor), and returns it at the revision stored.
    /// `Error::Conflict` when the stored mentor is at another revision, or a new one's id is taken.
    pub fn put_mentor(&self, mut m: Mentor) -> Result<Mentor> {
        m.rev = m.rev.saturating_add(1);
        m.rev = self.put_doc(
            concat!(
                "INSERT INTO mentors(id, created_at, doc) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET doc=excluded.doc",
                over_read_rev!("mentors"),
                " RETURNING json_extract(doc, '$.rev')"
            ),
            params![m.id, ms(m.created_at), doc(&m)?],
        )?;
        Ok(m)
    }

    /// Writes h over the revision it was read at, h.rev (0 for a new help), and returns it at the revision stored.
    /// `Error::Conflict` as put_mentor.
    pub fn put_help(&self, mut h: Help) -> Result<Help> {
        h.rev = h.rev.saturating_add(1);
        h.rev = self.put_doc(
            concat!(
                "INSERT INTO helps(id, chain, card, mentor, created_at, doc) VALUES(?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET doc=excluded.doc",
                over_read_rev!("helps"),
                " RETURNING json_extract(doc, '$.rev')"
            ),
            params![h.id, h.chain, h.card, h.mentor, ms(h.offered_at), doc(&h)?],
        )?;
        Ok(h)
    }
}

impl Reader<'_> {
    pub fn mentor(&self, id: &str) -> Result<Mentor> {
        self.one("SELECT doc FROM mentors WHERE id=?", [id])
    }

    /// Every mentor, the oldest first.
    pub fn mentors(&self) -> Result<Vec<Mentor>> {
        self.list("SELECT doc FROM mentors ORDER BY created_at, id", [])
    }

    pub fn help(&self, id: &str) -> Result<Help> {
        self.one("SELECT doc FROM helps WHERE id=?", [id])
    }

    /// The helps of chain chain_id, the oldest first.
    pub fn chain_helps(&self, chain_id: &str) -> Result<Vec<Help>> {
        self.list(
            "SELECT doc FROM helps WHERE chain=? ORDER BY created_at, id",
            [chain_id],
        )
    }

    /// The helps of card card_id, the oldest first.
    pub fn card_helps(&self, card_id: &str) -> Result<Vec<Help>> {
        self.list("SELECT doc FROM helps WHERE card=? ORDER BY created_at, id", [card_id])
    }

    /// The helps mentor was brought onto, the newest first: what their device lists and what a Chair reads of their
    /// record.
    pub fn mentor_helps(&self, mentor: &str) -> Result<Vec<Help>> {
        self.list(
            "SELECT doc FROM helps WHERE mentor=? ORDER BY created_at DESC, id DESC",
            [mentor],
        )
    }
}
