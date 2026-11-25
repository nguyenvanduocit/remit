//! How well memory is recalled (ADR-0117), measured each day by the job review_memories and recorded as the fact
//! memory.recall-quality, which the coach and any agent read with read_signal. Two measures, each blind where the other
//! sees:
//!
//! - **The week's use** (`week_quality`), from the facts agents leave: of the memories an agent said helped it, how many
//!   its turns were given unasked rather than found by searching (`recall_at_5`: a turn is given 5 at most); of those it
//!   was given and rated, how many helped (`precision`); and how often a session searched out a memory its turns were not
//!   given. It sees real work, and only what agents chose to rate.
//! - **The frozen questions** (`fixture_quality`): 17 messages written against the owner's memories (`recall_fixture.json`,
//!   12 with the memories that answer them, 5 about nothing they hold), each judged against every active memory as a
//!   turn's message is. It sees every memory, and only these questions. A question whose answers are no longer active is
//!   left out and counted, so the set reads true on any instance, and says so when it no longer bears on this one.
//!
//! A change to a model, a floor or a passage is measured by both before and after it. Pure but for `Crew::recall_quality`.

use std::collections::{HashMap, HashSet};

use model::{Card, Memory, Reading, Time};
use serde::{Deserialize, Serialize};

use crate::Crew;
use crate::memory::{ModelError, log_model_error, passage_of};
use crate::recall::{JUDGE_ALL, RERANK_MODEL, TURN_FLOOR, turn_queries};
use chains::fact::{FactValue, fact_value_of, fact_write, keys};

/// The frozen questions, written against the owner's memories on 2026-11-19.
const FIXTURE: &str = include_str!("recall_fixture.json");

/// How many of a question's best-judged memories count as given: what a turn is given at most.
const GIVEN: usize = 5;

/// How many days of facts the week's use reads: the days a fact is kept.
const WEEK_DAYS: i64 = 7;

/// How many of the newest recalls and uses are read: far more than an instance makes in a week.
const READ_MAX: i64 = 20_000;

/// One frozen question: a message, and the memories that answer it (none for a message about nothing they hold).
#[derive(Debug, Clone, Deserialize)]
struct Case {
    query: String,
    answers: Vec<String>,
}

/// What the week's facts say of recall.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct WeekQuality {
    /// Memories given to a turn unasked, one per card that was given each.
    pub given: usize,
    /// Memories an agent said helped it, one per card.
    pub helped: usize,
    /// Of those, how many its turns had been given, as a share of helped: none when nothing was said to help.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recall_at_5: Option<f64>,
    /// Of the memories given and rated, the share that helped: none when none was rated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub precision: Option<f64>,
    /// Memories a session searched out that its turns were not given, one per card.
    pub searched_not_given: usize,
}

/// What the frozen questions say of the judge and its floor.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct FixtureQuality {
    pub model: String,
    pub floor: f32,
    /// Questions judged: with every answer active, or about nothing.
    pub cases: usize,
    /// Questions left out: an answer is gone or no longer active here.
    pub left_out: usize,
    /// How many memories were judged per question, and whether that is every active one.
    pub judged: usize,
    pub all_judged: bool,
    /// Answers among their question's best `GIVEN`, of all answers, and how many fell under the floor.
    pub answers: usize,
    pub answers_in_top_5: usize,
    pub answers_under_floor: usize,
    /// The weakest answer's score, and the strongest any memory scored for a question about nothing they hold.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weakest_answer: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strongest_unrelated: Option<f32>,
    /// Memories that cleared the floor for a question about nothing they hold: what a turn would be given wrongly.
    pub unrelated_over_floor: usize,
}

/// The fact memory.recall-quality.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct RecallQuality {
    pub week: WeekQuality,
    /// None when the judge did not answer (no key, or it failed): `fixture_note` says why.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixture: Option<FixtureQuality>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub fixture_note: String,
}

/// n of d as a share, none when d is 0. Pure.
fn share(n: usize, d: usize) -> Option<f64> {
    let (n, d) = (u32::try_from(n).ok()?, u32::try_from(d).ok()?);
    (d > 0).then(|| f64::from(n) / f64::from(d))
}

/// What the readings of memory.recalled and memory.used since since say of recall (`WeekQuality`). Pure.
fn week_quality(recalled: &[Reading], used: &[Reading], since: Time) -> WeekQuality {
    let values = |rs: &[Reading]| -> Vec<FactValue> {
        rs.iter()
            .filter(|r| r.at >= since)
            .filter_map(|r| fact_value_of(r).ok())
            .collect()
    };
    let pairs = |v: &FactValue| {
        v.memories
            .iter()
            .map(|m| (v.card.clone(), m.clone()))
            .collect::<Vec<_>>()
    };
    let (mut given, mut searched) = (HashSet::new(), HashSet::new());
    for v in values(recalled) {
        let into = if v.searched { &mut searched } else { &mut given };
        into.extend(pairs(&v));
    }
    let (mut helped, mut misled) = (HashSet::new(), HashSet::new());
    for v in values(used) {
        let into = if v.memory_use.starts_with("helped") {
            &mut helped
        } else {
            &mut misled
        };
        into.extend(pairs(&v));
    }
    let helped_given = helped.intersection(&given).count();
    let misled_given = misled.intersection(&given).count();
    WeekQuality {
        given: given.len(),
        helped: helped.len(),
        recall_at_5: share(helped_given, helped.len()),
        precision: share(helped_given, helped_given.saturating_add(misled_given)),
        searched_not_given: searched.difference(&given).count(),
    }
}

/// The frozen questions. Pure.
fn cases() -> Vec<Case> {
    serde_json::from_str(FIXTURE).unwrap_or_default()
}

/// The questions that can be judged against the active memories (every answer active), and how many cannot. Pure.
fn judgeable(cases: Vec<Case>, active: &HashSet<&str>) -> (Vec<Case>, usize) {
    let (kept, left): (Vec<Case>, Vec<Case>) = cases
        .into_iter()
        .partition(|c| c.answers.iter().all(|a| active.contains(a.as_str())));
    (kept, left.len())
}

/// The memories each question is judged against: every active one while they are `JUDGE_ALL` or fewer, else every answer
/// and the latest changed of the rest. Pure.
fn pool<'a>(active: &'a [Memory], cases: &[Case]) -> Vec<&'a Memory> {
    let answers: HashSet<&str> = cases
        .iter()
        .flat_map(|c| c.answers.iter().map(String::as_str))
        .collect();
    let mut out: Vec<&Memory> = active.iter().filter(|m| answers.contains(m.id.as_str())).collect();
    let mut rest: Vec<&Memory> = active.iter().filter(|m| !answers.contains(m.id.as_str())).collect();
    rest.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then_with(|| a.id.cmp(&b.id)));
    out.extend(rest.into_iter().take(JUDGE_ALL.saturating_sub(out.len())));
    out
}

/// What the judge's scores of the pool's memories (by id) for each question say (`FixtureQuality`). Pure.
fn fixture_quality(
    cases: &[Case],
    scores: &[HashMap<String, f32>],
    left_out: usize,
    judged: usize,
    all_judged: bool,
) -> FixtureQuality {
    let mut q = FixtureQuality {
        model: RERANK_MODEL.to_owned(),
        floor: TURN_FLOOR,
        cases: cases.len(),
        left_out,
        judged,
        all_judged,
        ..FixtureQuality::default()
    };
    let lower = |a: Option<f32>, b: f32| Some(a.map_or(b, |a| a.min(b)));
    let higher = |a: Option<f32>, b: f32| Some(a.map_or(b, |a| a.max(b)));
    for (c, s) in cases.iter().zip(scores) {
        let mut order: Vec<(&String, f32)> = s.iter().map(|(id, v)| (id, *v)).collect();
        order.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        let top: HashSet<&str> = order.iter().take(GIVEN).map(|(id, _)| id.as_str()).collect();
        if c.answers.is_empty() {
            for (_, v) in &order {
                q.strongest_unrelated = higher(q.strongest_unrelated, *v);
                if *v >= TURN_FLOOR {
                    q.unrelated_over_floor = q.unrelated_over_floor.saturating_add(1);
                }
            }
            continue;
        }
        for a in &c.answers {
            let v = s.get(a).copied().unwrap_or(0.0);
            q.answers = q.answers.saturating_add(1);
            q.weakest_answer = lower(q.weakest_answer, v);
            if top.contains(a.as_str()) {
                q.answers_in_top_5 = q.answers_in_top_5.saturating_add(1);
            }
            if v < TURN_FLOOR {
                q.answers_under_floor = q.answers_under_floor.saturating_add(1);
            }
        }
    }
    q
}

impl Crew {
    /// Measures recall (`RecallQuality`) and records it as the fact memory.recall-quality, once a day: what the job
    /// review_memories runs after its reviews. A failure is logged; the measure is taken again the next day.
    pub(crate) async fn recall_quality(&self, active: &[Memory]) {
        let now = self.now();
        let since = Time(now.0.saturating_sub(time::Duration::days(WEEK_DAYS)));
        let read = self
            .store
            .read(|r| -> store::Result<_> {
                Ok((
                    r.newest_readings(&keys::memory_recalled(), READ_MAX)?,
                    r.newest_readings(&keys::memory_used(), READ_MAX)?,
                ))
            })
            .await;
        let (recalled, used) = match read {
            Ok(rs) => rs,
            Err(err) => {
                tracing::warn!(%err, "memory: the recalls to measure cannot be read");
                return;
            }
        };
        let mut quality = RecallQuality {
            week: week_quality(&recalled, &used, since),
            ..RecallQuality::default()
        };
        match self.fixture_scores(active).await {
            Ok(q) => quality.fixture = Some(q),
            Err(err) => {
                log_model_error("measure recall", &err);
                quality.fixture_note = format!("the frozen questions were not judged: {err}");
            }
        }
        let day = now.0.date();
        let w = match fact_write(keys::memory_recall_quality(), &quality, model::by_remit("memory"), "") {
            Ok(mut w) => {
                w.dedupes = vec![format!("recall-quality:{day}")];
                w
            }
            Err(err) => {
                tracing::error!(%err, "memory: the recall quality is not recorded");
                return;
            }
        };
        let door = self.door();
        if let Err(err) = self.store.update(move |tx| door.ingest(tx, w).map(|_| ())).await {
            tracing::error!(%err, "memory: the recall quality is not recorded");
            return;
        }
        self.kick();
    }

    /// The judge's scores for each frozen question that can be judged here, against the active memories (`pool`), as a
    /// turn's message is judged.
    async fn fixture_scores(&self, active: &[Memory]) -> std::result::Result<FixtureQuality, ModelError> {
        let ids: HashSet<&str> = active.iter().map(|m| m.id.as_str()).collect();
        let (cases, left_out) = judgeable(cases(), &ids);
        let pool = pool(active, &cases);
        let docs: Vec<String> = pool.iter().map(|m| passage_of(m)).collect();
        let mut scores = Vec::new();
        for c in &cases {
            let judged = turn_queries(&Card::default(), &c.query).judged;
            let s = self.models.rerank(RERANK_MODEL, &judged, &docs).await?;
            scores.push(pool.iter().map(|m| m.id.clone()).zip(s).collect());
        }
        Ok(fixture_quality(
            &cases,
            &scores,
            left_out,
            pool.len(),
            pool.len() == active.len(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frozen_questions_read() {
        let all = cases();
        assert_eq!(all.len(), 17);
        assert_eq!(all.iter().filter(|c| c.answers.is_empty()).count(), 5);
    }
}
