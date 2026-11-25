//! An objective reaches the owner as a change item (ADR-0031). A Chair drafts one and the Assistant applies it for him,
//! which makes it active; the Assistant drafts its close the same way. Only the owner's side applies either (`for_owner`,
//! `answer_change`).

use std::collections::BTreeMap;

use model::{ChangeKind, ChangedField, Employee, Horizon, Objective, ObjectiveStatus, PendingChange, Surrogate, Time};
use serde::{Deserialize, Serialize};
use store::Tx;

use crate::Crew;
use crate::objective::{checked_objective, findings_words};
use chains::chain_core::optional;
use chains::change_item::{Applied, Tell};
use chains::changes::{Authorship, record_change};
use chains::goal::check_words;
use chains::owner_acts::{ACT_APPLY, for_owner};
use chains::{Error, Result};

/// What a change item of kind objective carries: the objective as the owner is shown it, and the employees the draft
/// proposes to hire with it (ADR-0032). Against a draft it is the draft itself; against an active objective it is either
/// the proposal of an amendment, over the revision it was made over (`Objective::rev`), or, with closes, the objective to
/// close.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ObjectiveRecord {
    pub objective: Objective,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hires: Vec<Employee>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub closes: bool,
}

impl ObjectiveRecord {
    /// The objective stored over cur when the owner applies the record: a draft is activated, an active objective is
    /// closed or replaced by the amendment (revision read, status active), and None for what nothing applies to (a closed
    /// or a task objective, a draft that is asked to close).
    fn applied_over(&self, cur: &Objective) -> Option<Objective> {
        if cur.task {
            return None;
        }
        let mut applied = cur.clone();
        match (&cur.status, self.closes) {
            (ObjectiveStatus::Draft, false) => applied.status = ObjectiveStatus::Active,
            (ObjectiveStatus::Active, true) => applied.status = ObjectiveStatus::Closed,
            (ObjectiveStatus::Active, false) => {
                applied = Objective {
                    rev: cur.rev,
                    status: ObjectiveStatus::Active,
                    created_at: cur.created_at,
                    ..self.objective.clone()
                };
            }
            _ => return None,
        }
        Some(applied)
    }
}

/// What the owner reads and answers for an objective, one field each. For a draft or a close it is the status change, the
/// version, what the objective says and the hires it proposes; for an amendment, the version and only what the proposal
/// changes, each as it is and as it would be. It is derived from the stored objective, so an objective written since the
/// owner was asked gives other fields and the answer is refused as moved.
pub(crate) fn objective_rail(cur: &Objective, rec: &ObjectiveRecord) -> Vec<ChangedField> {
    let applied = rec.applied_over(cur).unwrap_or_else(|| cur.clone());
    let version = ChangedField {
        name: "version".to_owned(),
        before: String::new(),
        after: json_text(&cur.rev.to_string()),
    };
    if applied.status == cur.status {
        // an amendment
        let mut out = vec![version];
        out.extend(changes_between(
            &describe_objective(cur, &[]),
            &describe_objective(&applied, &rec.hires),
        ));
        return out;
    }
    let status = ChangedField {
        name: "status".to_owned(),
        before: json_text(cur.status.as_str()),
        after: json_text(applied.status.as_str()),
    };
    let mut out = vec![status, version];
    out.extend(describe_objective(cur, &rec.hires));
    out
}

/// What an objective holds, one field each, for the owner to read.
fn describe_objective(o: &Objective, hires: &[Employee]) -> Vec<ChangedField> {
    let mut fields = Vec::new();
    let mut add = |name: String, value: String| {
        if !value.is_empty() {
            fields.push(ChangedField {
                name,
                before: String::new(),
                after: json_text(&value),
            });
        }
    };
    add("scope".to_owned(), o.scope.clone());
    add("customer".to_owned(), o.customer.clone());
    add("project".to_owned(), o.project.clone().unwrap_or_default());
    add("outcome".to_owned(), o.outcome.clone());
    add("north star".to_owned(), o.north_star.clone());
    add("horizon".to_owned(), horizon_words(&o.horizon));
    add("owner".to_owned(), o.owner.to_string());
    add(
        "guardrails".to_owned(),
        list_words(&o.guardrails, |g| {
            format!("{}: {} on the signal {}", g.kpi, g.condition, g.signal)
        }),
    );
    add("surrogates".to_owned(), list_words(&o.surrogates, surrogate_words));
    add(
        "unknown".to_owned(),
        list_words(&o.unknown, |u| format!("{} {}", u.text, u.question).trim().to_owned()),
    );
    for kr in &o.key_results {
        let mut words = format!(
            "{}\nowner: {}\njudge: {}\ncheck: {}",
            kr.text,
            kr.owner,
            kr.judge,
            check_words(kr.check.as_ref())
        );
        if kr.retired {
            words.push_str("\nretired: its chains finish and no new chain opens for it");
        }
        add(format!("key result {}", kr.id), words);
    }
    for h in hires {
        add(
            format!("hire {}", h.id),
            format!(
                "{} {}, from the template {}, scope: {}",
                h.title, h.label, h.template, h.scope
            ),
        );
    }
    fields
}

/// The fields of after that differ from before, each with what it was, and the fields that after no longer has.
fn changes_between(before: &[ChangedField], after: &[ChangedField]) -> Vec<ChangedField> {
    let mut was: BTreeMap<&str, &str> = before.iter().map(|f| (f.name.as_str(), f.after.as_str())).collect();
    let mut out = Vec::new();
    for f in after {
        let old = was.remove(f.name.as_str());
        if old != Some(f.after.as_str()) {
            out.push(ChangedField {
                name: f.name.clone(),
                before: old.unwrap_or_default().to_owned(),
                after: f.after.clone(),
            });
        }
    }
    for f in before {
        if let Some(old) = was.remove(f.name.as_str()) {
            out.push(ChangedField {
                name: f.name.clone(),
                before: old.to_owned(),
                after: String::new(),
            });
        }
    }
    out
}

/// s as a JSON string, the shape a field's value has (`plain_json` reads it back for the owner).
fn json_text(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

/// The day of t, as the owner reads it: 2026-11-11.
fn day_words(t: Time) -> String {
    let d = t.0.date();
    format!("{:04}-{:02}-{:02}", d.year(), u8::from(d.month()), d.day())
}

fn horizon_words(h: &Horizon) -> String {
    match (h.text.is_empty(), h.until) {
        (false, Some(until)) => format!("{} (until {})", h.text, day_words(until)),
        (false, None) => h.text.clone(),
        (true, Some(until)) => format!("until {}", day_words(until)),
        (true, None) => String::new(),
    }
}

/// A surrogate in one sentence (ADR-0081): what it stands in for, what it reads, who answers for it and until when.
fn surrogate_words(s: &Surrogate) -> String {
    let mut words = format!("{} stands in", s.kpi);
    if !s.reads.is_empty() {
        words.push_str(&format!(", read from {}", s.reads));
    }
    if !s.owner.0.is_empty() {
        words.push_str(&format!(", answered for by {}", s.owner));
    }
    if let Some(expires) = s.expires {
        words.push_str(&format!(", until {}", day_words(expires)));
    }
    words
}

fn list_words<T>(items: &[T], word: impl Fn(&T) -> String) -> String {
    items
        .iter()
        .map(|it| format!("- {}", word(it)))
        .collect::<Vec<_>>()
        .join("\n")
}

impl Crew {
    /// Stores the objective a change item asks to write, if the stored one is still as the owner was shown, and opens its
    /// work: the hires it proposes (`apply_proposed_hires`) in the same transaction, the key-result owners' chains
    /// (`wake_key_result_owners`) once it is committed. A draft becomes active, an active objective is closed or amended,
    /// written over the revision read; a second answer to the same item never reaches here (the item is closed), and a
    /// replay finds the objective in its new revision, which is moved.
    pub fn apply_objective(&self, tx: &Tx<'_>, pending: &PendingChange, w: &Authorship) -> Result<Applied> {
        if !w
            .approved_by
            .as_ref()
            .is_some_and(|by| for_owner(ACT_APPLY, by).is_ok())
        {
            return Err(Error::refused(ACT_APPLY.refusal()));
        }
        let rec: ObjectiveRecord = serde_json::from_str(&pending.record)?;
        let Some(cur) = optional(tx.objective(&rec.objective.id))? else {
            return Ok(Applied::Moved);
        };
        if objective_rail(&cur, &rec) != pending.fields {
            return Ok(Applied::Moved);
        }
        let Some(applied) = rec.applied_over(&cur) else {
            return Ok(Applied::Moved);
        };
        let now = self.now();
        if !rec.closes {
            // The dry run is the gate (ADR-0045): a measure counts only when its signal reads it now, so the owner
            // applies nothing the checker still finds fault with, and the signal is read again here because it may have
            // changed since the draft.
            let found = checked_objective(tx, &applied, now)?;
            if !found.is_empty() {
                return Err(Error::refused(format!(
                    "the objective cannot be applied yet: its measures do not pass the dry run.{}",
                    findings_words(&found)
                )));
            }
        }
        // cur was read in tx, so the write over its revision is not refused
        let applied = tx.put_objective(applied)?;
        record_change(
            tx,
            w,
            now,
            ChangeKind::Objective,
            &applied.id,
            Some(&cur),
            Some(&applied),
        )?;
        let active = applied.status == ObjectiveStatus::Active;
        let hired: Option<Tell> = if active {
            self.apply_proposed_hires(tx, &applied, &rec.hires, w)?
        } else {
            None
        };
        let crew = self.me();
        Ok(Applied::Written(Some(Box::new(move |o: &chains::Core| {
            if let Some(hired) = hired {
                hired(o);
            }
            let Some(me) = crew else {
                return;
            };
            o.work.spawn(async move {
                me.tell_objective(&applied.id).await;
                if active {
                    me.wake_key_result_owners(&applied).await;
                }
            });
        }))))
    }

    /// Pushes the objective id, as stored, with where its key results stand.
    pub async fn tell_objective(&self, id: &str) {
        match self.objective_views().await {
            Ok(views) => {
                for v in views.iter().filter(|v| v.objective.id == id) {
                    self.notify.objective_updated(v);
                }
            }
            Err(err) => tracing::warn!(objective = %id, %err, "tell the channels of an objective"),
        }
    }
}
