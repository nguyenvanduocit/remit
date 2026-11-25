//! Judgments (ADR-0118, migration 126): one decider's answer to one typed question about one state, one row each,
//! written once. The outcome a pick proved is set on every judgment of its asking at once, so the acting decider and its
//! shadows are scored on the same event; a shadow stored after it takes it as it is stored. A judgment names a subject,
//! not a home: a decider's standing outlives the chain it was asked in.

use model::{Judgment, JudgmentOutcome, JudgmentUse, OutcomeSource, Proved};
use rusqlite::params;

use crate::store::{Error, Reader, Result, Tx, doc, ms};

/// A judgment with a known outcome, as its decider is scored on it.
#[derive(Debug, Clone, PartialEq)]
pub struct JudgmentScore {
    pub question: String,
    pub decider: String,
    /// Its probability of its asking's pick.
    pub p: f64,
    /// Whether the pick proved right.
    pub right: bool,
    /// What it picked, as the kind of its pick: `yes`, `no`, `option` or `level`.
    pub picked: String,
}

/// How many judgments one decider gave one question that the code made one use of, and what they cost.
#[derive(Debug, Clone, PartialEq)]
pub struct JudgmentTally {
    pub question: String,
    pub decider: String,
    pub used: JudgmentUse,
    pub judgments: i64,
    /// Of them, those whose pick proved right, proved wrong, and whose outcome can no longer be known.
    pub right: i64,
    pub wrong: i64,
    pub unknown: i64,
    /// In US dollars.
    pub cost_usd: f64,
}

impl Tx<'_> {
    /// Writes each of judgments once (one stored already is left as it is), each with its asking's outcome when one is
    /// known.
    pub fn put_judgments(&self, judgments: Vec<Judgment>) -> Result<()> {
        for mut j in judgments {
            if j.outcome.is_none() {
                j.outcome = self.asking_outcome(&j.asking)?;
            }
            self.exec(
                "INSERT INTO judgments(id, asking, question, decider, created_at, doc) VALUES(?,?,?,?,?,?) ON CONFLICT(id) DO NOTHING",
                params![j.id, j.asking, j.question, j.decider, ms(j.created_at), doc(&j)?],
            )?;
        }
        Ok(())
    }

    /// Records outcome on the judgment judgment_id and every other judgment of its asking; recorded again, it replaces
    /// the one before. `Error::NotFound` when no such judgment is stored.
    pub fn set_judgment_outcome(&self, judgment_id: &str, outcome: &JudgmentOutcome) -> Result<()> {
        let changed = self.exec(
            "UPDATE judgments SET doc = json_set(doc, '$.outcome', json(?)) WHERE asking = (SELECT asking FROM judgments WHERE id = ?)",
            params![doc(outcome)?, judgment_id],
        )?;
        if changed == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }
}

impl Tx<'_> {
    /// Withdraws the outcome recorded on the asking of judgment_id and every judgment of it: what proved it is no longer
    /// known to. `Error::NotFound` when no such judgment is stored.
    pub fn clear_judgment_outcome(&self, judgment_id: &str) -> Result<()> {
        let changed = self.exec(
            "UPDATE judgments SET doc = json_remove(doc, '$.outcome') WHERE asking = (SELECT asking FROM judgments WHERE id = ?)",
            params![judgment_id],
        )?;
        if changed == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Records outcome on the asking of judgment_id as `set_judgment_outcome` does, but only when it has none yet: what
    /// is known of a pick whose subject ended before it showed must not replace what an earlier event proved.
    pub fn settle_judgment_outcome(&self, judgment_id: &str, outcome: &JudgmentOutcome) -> Result<()> {
        self.exec(
            "UPDATE judgments SET doc = json_set(doc, '$.outcome', json(?)) WHERE proved IS NULL AND asking = (SELECT asking FROM judgments WHERE id = ?)",
            params![doc(outcome)?, judgment_id],
        )?;
        Ok(())
    }

    /// Records outcome on the asking of judgment_id as `set_judgment_outcome` does, but writes nothing when the asking
    /// already holds that outcome from that source: a write that is replayed leaves when it was first known as it was.
    pub fn change_judgment_outcome(&self, judgment_id: &str, outcome: &JudgmentOutcome) -> Result<()> {
        self.exec(
            "UPDATE judgments SET doc = json_set(doc, '$.outcome', json(?1))
             WHERE asking = (SELECT asking FROM judgments WHERE id = ?2)
               AND (proved IS NOT ?3 OR json_extract(doc, '$.outcome.source') IS NOT ?4)",
            params![
                doc(outcome)?,
                judgment_id,
                outcome.proved.as_str(),
                outcome.source.as_str()
            ],
        )?;
        Ok(())
    }

    /// Records that the code made used of judgment judgment_id: a judgment the code acted on though it was below its act
    /// threshold (an exploration). `Error::NotFound` when no such judgment is stored.
    pub fn set_judgment_used(&self, judgment_id: &str, used: JudgmentUse) -> Result<()> {
        let changed = self.exec(
            "UPDATE judgments SET doc = json_set(doc, '$.used', ?) WHERE id = ?",
            params![used.as_str(), judgment_id],
        )?;
        if changed == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }
}

impl Reader<'_> {
    /// The judgment judgment_id. `Error::NotFound` when no such judgment is stored.
    pub fn judgment(&self, judgment_id: &str) -> Result<Judgment> {
        self.one("SELECT doc FROM judgments WHERE id = ?", [judgment_id])
    }

    /// The outcome recorded on asking, none before one is.
    fn asking_outcome(&self, asking: &str) -> Result<Option<JudgmentOutcome>> {
        let text: Option<String> = self
            .column(
                "SELECT json_extract(doc, '$.outcome') FROM judgments WHERE asking = ? AND proved IS NOT NULL LIMIT 1",
                [asking],
            )?
            .into_iter()
            .next();
        text.map(|t| serde_json::from_str(&t).map_err(Error::from)).transpose()
    }

    /// The judgments that proved right or wrong of the questions named (every question's when none is named), as each
    /// decider is scored on them.
    pub fn judgment_scores(&self, questions: &[String]) -> Result<Vec<JudgmentScore>> {
        let mut stmt = self.c.prepare_cached(
            "SELECT question, decider, json_extract(doc, '$.p'), proved, json_extract(doc, '$.pick.type') FROM judgments
             WHERE proved IN (?2, ?3) AND json_extract(doc, '$.outcome.source') != ?4
               AND (?1 = '[]' OR question IN (SELECT value FROM json_each(?1)))",
        )?;
        let rows = stmt.query_map(
            params![
                serde_json::to_string(questions)?,
                Proved::Right.as_str(),
                Proved::Wrong.as_str(),
                OutcomeSource::SecondOpinionTargeted.as_str()
            ],
            |r| {
                Ok(JudgmentScore {
                    question: r.get(0)?,
                    decider: r.get(1)?,
                    p: r.get(2)?,
                    right: r.get::<_, String>(3)? == Proved::Right.as_str(),
                    picked: r.get(4)?,
                })
            },
        )?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// How many judgments each decider gave each question, by the use the code made of them, with their cost: the
    /// questions asked last first.
    pub fn judgment_tallies(&self) -> Result<Vec<JudgmentTally>> {
        let mut stmt = self.c.prepare_cached(
            "SELECT question, decider, used, count(*), count(*) FILTER (WHERE proved = ?1),
                    count(*) FILTER (WHERE proved = ?2), count(*) FILTER (WHERE proved = ?3),
                    total(json_extract(doc, '$.costUsd')), max(max(created_at)) OVER (PARTITION BY question) AS last
             FROM judgments GROUP BY question, decider, used ORDER BY last DESC, question, decider, used",
        )?;
        let rows = stmt.query_map(
            [Proved::Right.as_str(), Proved::Wrong.as_str(), Proved::Unknown.as_str()],
            |r| {
                Ok((
                    (r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?),
                    (
                        r.get::<_, i64>(3)?,
                        r.get::<_, i64>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, i64>(6)?,
                    ),
                    r.get::<_, f64>(7)?,
                ))
            },
        )?;
        let mut out = Vec::new();
        for row in rows {
            let ((question, decider, used), (judgments, right, wrong, unknown), cost_usd) = row?;
            match used.parse::<JudgmentUse>() {
                Ok(used) => out.push(JudgmentTally {
                    question,
                    decider,
                    used,
                    judgments,
                    right,
                    wrong,
                    unknown,
                    cost_usd,
                }),
                Err(why) => {
                    self.left_out("JudgmentTally", &question, &why);
                }
            }
        }
        Ok(out)
    }

    /// The judgments with these ids, those stored: what a point that kept their ids reads to record what they proved.
    pub fn judgments_named(&self, ids: &[String]) -> Result<Vec<Judgment>> {
        self.list(
            "SELECT doc FROM judgments WHERE id IN (SELECT value FROM json_each(?1)) ORDER BY created_at, id",
            params![serde_json::to_string(ids)?],
        )
    }

    /// The judgments of questions that the code used (no shadow's) about a pair of memories one of which is memory, newest
    /// first. A pair's id joins its two memories' ids with `+`, so each is matched whole.
    pub fn pair_judgments_of(&self, memory: &str, questions: &[String]) -> Result<Vec<Judgment>> {
        self.list(
            "SELECT doc FROM judgments
             WHERE question IN (SELECT value FROM json_each(?1)) AND used != ?2
               AND json_extract(doc, '$.subject.kind') = 'memory_pair'
               AND instr('+' || json_extract(doc, '$.subject.id') || '+', '+' || ?3 || '+') > 0
             ORDER BY created_at DESC, id DESC",
            params![serde_json::to_string(questions)?, JudgmentUse::Shadow.as_str(), memory],
        )
    }

    /// The judgments of questions that the code used (no shadow's) about the inbox item item, newest first: an item put
    /// to the Assistant twice (a firing delivered again) has two askings, and the newest is the one it was shown.
    pub fn item_judgments_of(&self, item: &str, questions: &[String]) -> Result<Vec<Judgment>> {
        self.list(
            "SELECT doc FROM judgments
             WHERE question IN (SELECT value FROM json_each(?1)) AND used != ?2
               AND json_extract(doc, '$.subject.kind') = 'inbox_item' AND json_extract(doc, '$.subject.id') = ?3
             ORDER BY created_at DESC, id DESC",
            params![serde_json::to_string(questions)?, JudgmentUse::Shadow.as_str(), item],
        )
    }

    /// The newest judgment of question that the code used (no shadow's) about the reading subject, if any: what a screen
    /// that is asked the same reading again reads instead of asking twice. No index holds it yet; revisit when judgments
    /// are many (ADR-0118, Revisit when).
    pub fn reading_judgment(&self, question: &str, subject: &str) -> Result<Option<Judgment>> {
        let mut found: Vec<Judgment> = self.list(
            "SELECT doc FROM judgments
             WHERE question = ? AND used != ? AND json_extract(doc, '$.subject.kind') = 'reading'
               AND json_extract(doc, '$.subject.id') = ?
             ORDER BY created_at DESC, id DESC LIMIT 1",
            params![question, JudgmentUse::Shadow.as_str(), subject],
        )?;
        Ok(found.pop())
    }

    /// The judgment the firing of subscription sub that started chain carries, if it carries one: what a chain finishing
    /// scores. The firing owns the link, the chain is found by it.
    pub fn screen_of_start(&self, sub: &str, chain: &str) -> Result<Option<String>> {
        let mut found: Vec<String> = self.column(
            "SELECT json_extract(doc, '$.screen') FROM firings
             WHERE subscription_id = ? AND json_extract(doc, '$.chain') = ? AND json_extract(doc, '$.screen') != ''
             LIMIT 1",
            params![sub, chain],
        )?;
        Ok(found.pop())
    }

    /// The judgments of question that acted on a yes whose outcome is not known yet, oldest first: what the daily review
    /// reads of the contradictions the deciders found.
    pub fn open_yes_judgments(&self, question: &str) -> Result<Vec<Judgment>> {
        self.list(
            "SELECT doc FROM judgments
             WHERE question = ? AND used = ? AND proved IS NULL AND json_extract(doc, '$.pick.type') = 'yes'
             ORDER BY created_at, id",
            params![question, JudgmentUse::Acted.as_str()],
        )
    }

    /// The judgments of questions that the code used (no shadow's) about the reports of card card_id whose pick has no
    /// outcome yet: what the Chair's verdict on a report proves of the watchdog's answers about it, and what an older
    /// report's answers are left without.
    pub fn open_report_judgments(&self, card_id: &str, questions: &[String]) -> Result<Vec<Judgment>> {
        self.list(
            "SELECT doc FROM judgments
             WHERE question IN (SELECT value FROM json_each(?1)) AND used != ?2 AND proved IS NULL
               AND json_extract(doc, '$.subject.kind') = 'report' AND json_extract(doc, '$.subject.id') GLOB ?3
             ORDER BY created_at, id",
            params![
                serde_json::to_string(questions)?,
                JudgmentUse::Shadow.as_str(),
                format!("{card_id}#*")
            ],
        )
    }

    /// The newest judgments of question the code used whose pick proved wrong, at most limit.
    pub fn wrong_judgments(&self, question: &str, limit: i64) -> Result<Vec<Judgment>> {
        self.list(
            "SELECT doc FROM judgments WHERE question = ? AND used != ? AND proved = ? ORDER BY created_at DESC, id DESC LIMIT ?",
            params![
                question,
                JudgmentUse::Shadow.as_str(),
                Proved::Wrong.as_str(),
                limit
            ],
        )
    }
}
