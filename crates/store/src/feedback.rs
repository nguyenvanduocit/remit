//! Feedback: what agents tell the organisation about how they work. It is written once and then only marked read, so it
//! is never pruned with its chain: the owner reads it long after the chain is archived.

use model::Feedback;
use rusqlite::params;

use crate::store::{Reader, Result, Tx, doc, ms, over_read_rev};

impl Tx<'_> {
    /// Writes f over the revision it was read at, f.rev (0 for new feedback), and returns it at the revision stored.
    /// `Error::Conflict` when the stored feedback is at another revision, or a new one's id is taken.
    pub fn put_feedback(&self, mut f: Feedback) -> Result<Feedback> {
        f.rev = f.rev.saturating_add(1);
        f.rev = self.put_doc(
            concat!(
                "INSERT INTO feedback(id, card_id, created_at, doc) VALUES(?,?,?,?) ON CONFLICT(id) DO UPDATE SET doc=excluded.doc",
                over_read_rev!("feedback"),
                " RETURNING json_extract(doc, '$.rev')"
            ),
            params![f.id, f.card_id, ms(f.created_at), doc(&f)?],
        )?;
        Ok(f)
    }
}

impl Reader<'_> {
    pub fn feedback(&self, id: &str) -> Result<Feedback> {
        self.one("SELECT doc FROM feedback WHERE id=?", [id])
    }

    /// The newest limit pieces of feedback, the newest first.
    pub fn recent_feedback(&self, limit: i64) -> Result<Vec<Feedback>> {
        self.list(
            "SELECT doc FROM feedback ORDER BY created_at DESC, id DESC LIMIT ?",
            [limit],
        )
    }

    /// How many pieces of feedback the card has sent.
    pub fn feedback_of_card(&self, card_id: &str) -> Result<i64> {
        self.value("SELECT count(*) FROM feedback WHERE card_id=?", [card_id])
    }
}
