//! Objectives: what the owner wants reached, as key results (ADR-0031). One document with a revision, like a chain; its
//! key results are an entity of their own (ADR-0115), one row each in `key_results`, keyed by their objective and their
//! id: the ones with no parent, the owner's commitment, are written and read with the objective in their place among
//! his; the ones that refine them on their own. The last lift of a key result's progress rail is a column of its own,
//! written only with `put_key_result`, so writing the commitment again keeps it.

use std::collections::HashMap;

use model::{KeyResult, Objective};
use rusqlite::params;

use crate::store::{Reader, Result, Tx, doc, ms, over_read_rev};

/// A key result as a row reads it: the parent and the lift are the row's columns, not its document's.
const KEY_RESULT: &str = "json_set(doc, '$.parent', parent, '$.lifted', json(lifted))";

/// The order key results are listed in: the owner's in his order, then the ones that refine others in the order they
/// were written, so a parent always comes before what refines it.
const IN_ORDER: &str = "ORDER BY parent IS NOT NULL, place, rowid";

impl Tx<'_> {
    /// Writes o over the revision it was read at, o.rev (0 for a new objective), and returns it at the revision stored.
    /// Its key results are written as rows with no parent, in their order, and not in its document; a committed key
    /// result it no longer names is deleted (a redraft drops it; an amendment keeps what it drops as retired). A key
    /// result's lift is kept. `Error::Conflict` when the stored objective is at another revision, or a new one's id is
    /// taken.
    pub fn put_objective(&self, mut o: Objective) -> Result<Objective> {
        o.rev = o.rev.saturating_add(1);
        let key_results = std::mem::take(&mut o.key_results);
        o.rev = self.put_doc(
            concat!(
                "INSERT INTO objectives(id, created_at, doc) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET doc=excluded.doc",
                over_read_rev!("objectives"),
                " RETURNING json_extract(doc, '$.rev')"
            ),
            params![o.id, ms(o.created_at), doc(&o)?],
        )?;
        let stored: Vec<String> = self.list(
            "SELECT json_quote(id) FROM key_results WHERE objective=? AND parent IS NULL",
            [&o.id],
        )?;
        for id in stored.iter().filter(|id| !key_results.iter().any(|kr| &kr.id == *id)) {
            self.exec("DELETE FROM key_results WHERE objective=? AND id=?", params![o.id, id])?;
        }
        for (place, kr) in key_results.into_iter().enumerate() {
            let kr = KeyResult {
                parent: None,
                lifted: None,
                ..kr
            };
            self.exec(
                "INSERT INTO key_results(objective, id, parent, place, doc) VALUES(?,?,NULL,?,?) \
                 ON CONFLICT(objective, id) DO UPDATE SET parent=NULL, place=excluded.place, doc=excluded.doc",
                params![o.id, kr.id, i64::try_from(place).unwrap_or(i64::MAX), doc(&kr)?],
            )?;
        }
        o.key_results = self.committed_key_results(&o.id)?;
        Ok(o)
    }

    /// Writes key result kr of objective, a new one or over the one stored with its id (its parent and its lift
    /// included), and returns it. A key result keeps its place among its objective's on every write.
    pub fn put_key_result(&self, objective: &str, kr: KeyResult) -> Result<KeyResult> {
        let lifted = kr.lifted.as_ref().map(doc).transpose()?;
        let body = KeyResult {
            parent: None,
            lifted: None,
            ..kr.clone()
        };
        self.exec(
            "INSERT INTO key_results(objective, id, parent, lifted, doc) VALUES(?,?,?,?,?) \
             ON CONFLICT(objective, id) DO UPDATE SET parent=excluded.parent, lifted=excluded.lifted, doc=excluded.doc",
            params![objective, kr.id, kr.parent, lifted, doc(&body)?],
        )?;
        Ok(kr)
    }
}

impl Reader<'_> {
    pub fn objective(&self, id: &str) -> Result<Objective> {
        let mut o: Objective = self.one("SELECT doc FROM objectives WHERE id=?", [id])?;
        o.key_results = self.committed_key_results(id)?;
        Ok(o)
    }

    /// Every objective, newest first, each with the key results the owner committed to.
    pub fn objectives(&self) -> Result<Vec<Objective>> {
        let mut all: Vec<Objective> = self.list("SELECT doc FROM objectives ORDER BY created_at DESC, id", [])?;
        let mut trees = self.every_key_result()?;
        for o in &mut all {
            o.key_results = trees
                .remove(&o.id)
                .unwrap_or_default()
                .into_iter()
                .filter(|kr| kr.parent.is_none())
                .collect();
        }
        Ok(all)
    }

    /// The key results of objective with no parent, in the owner's order.
    fn committed_key_results(&self, objective: &str) -> Result<Vec<KeyResult>> {
        self.list(
            &format!("SELECT {KEY_RESULT} FROM key_results WHERE objective=? AND parent IS NULL {IN_ORDER}"),
            [objective],
        )
    }

    /// Every key result of objective, those that refine others included, parents before what refines them.
    pub fn key_results(&self, objective: &str) -> Result<Vec<KeyResult>> {
        self.list(
            &format!("SELECT {KEY_RESULT} FROM key_results WHERE objective=? {IN_ORDER}"),
            [objective],
        )
    }

    /// Key result id of objective, whether the owner committed to it or it refines another.
    pub fn key_result(&self, objective: &str, id: &str) -> Result<KeyResult> {
        self.one(
            &format!("SELECT {KEY_RESULT} FROM key_results WHERE objective=? AND id=?"),
            [objective, id],
        )
    }

    /// Every key result of every objective, by objective id, each objective's parents before what refines them.
    pub fn every_key_result(&self) -> Result<HashMap<String, Vec<KeyResult>>> {
        let rows: Vec<Placed> = self.list(
            &format!("SELECT json_set({KEY_RESULT}, '$.objective', objective) FROM key_results {IN_ORDER}"),
            [],
        )?;
        let mut by: HashMap<String, Vec<KeyResult>> = HashMap::new();
        for row in rows {
            by.entry(row.objective).or_default().push(row.key_result);
        }
        Ok(by)
    }
}

/// A key result read with the objective its row names.
#[derive(serde::Deserialize)]
struct Placed {
    objective: String,
    #[serde(flatten)]
    key_result: KeyResult,
}
