//! Second opinions (migration 107, ADR-0103): a fresh judge's re-decision of one of the Assistant's answers, one row each,
//! its judge's card a column so the judge's tools find the one it gives.

use model::SecondOpinion;
use rusqlite::params;

use crate::store::{Reader, Result, Tx, doc, ms, over_read_rev};

impl Tx<'_> {
    /// Writes s over the revision it was read at, s.rev (0 for a new one), and returns it at the revision stored.
    /// `Error::Conflict` when the stored one is at another revision, or a new one's id is taken.
    pub fn put_second_opinion(&self, mut s: SecondOpinion) -> Result<SecondOpinion> {
        s.rev = s.rev.saturating_add(1);
        s.rev = self.put_doc(
            concat!(
                "INSERT INTO second_opinions(id, chain, judge, created_at, doc) VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET doc=excluded.doc",
                over_read_rev!("second_opinions"),
                " RETURNING json_extract(doc, '$.rev')"
            ),
            params![s.id, s.chain, s.judge, ms(s.created_at), doc(&s)?],
        )?;
        Ok(s)
    }
}

impl Reader<'_> {
    /// The second opinion judge_id gives.
    pub fn second_opinion_of(&self, judge_id: &str) -> Result<SecondOpinion> {
        self.one("SELECT doc FROM second_opinions WHERE judge=?", [judge_id])
    }

    /// The second opinions on the inbox item item.
    pub fn second_opinions_of_item(&self, item: &str) -> Result<Vec<SecondOpinion>> {
        self.list(
            "SELECT doc FROM second_opinions WHERE json_extract(doc, '$.item') = ?",
            params![item],
        )
    }

    /// The newest second opinions, at most limit, the newest first.
    pub fn recent_second_opinions(&self, limit: i64) -> Result<Vec<SecondOpinion>> {
        self.list(
            "SELECT doc FROM second_opinions ORDER BY created_at DESC, id DESC LIMIT ?",
            params![limit],
        )
    }
}
