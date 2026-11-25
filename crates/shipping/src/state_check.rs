//! The check a direct request is judged by (ADR-0031, ADR-0068): its Chair states how the result will be checked, and
//! the goal judge rules on that check with the result when the Chair asks to finish (goal.rs).

use model::{Card, Check, CheckKind, DecisionKind, Finding, Objective, Origin, by_agent};

use crate::Shipping;
use chains::chain_core::optional;
use chains::changes::Authorship;
use chains::fact::{FactValue, fact_write, keys};
use chains::goal::finding_lines;
use chains::schedule::verifying;
use chains::{Error, Result};
use crew::key_result_tools::write_key_result;

/// The decision the owner reads on the chain when its Chair declares the check, as text and why: what the check is, in a
/// few words, and what he needs to trust it. A submit or an unverifiable check says why nothing can run; a command or a
/// reading says what it is seen through once live and what happens if it fails there.
pub(crate) fn check_decision(c: &Check) -> (String, String) {
    let text = format!("Check: {}.", check_target(c));
    let why = match c.kind {
        CheckKind::Submit | CheckKind::Unverifiable => c.reason.clone(),
        _ => [("Seen through", &c.telemetry), ("If it fails live", &c.incident)]
            .into_iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, v)| format!("{k}: {v}."))
            .collect::<Vec<_>>()
            .join(" "),
    };
    (text, why)
}

/// What a check runs or reads, in a few words.
pub(crate) fn check_target(c: &Check) -> String {
    match c.kind {
        CheckKind::Command => format!("`{}` in {}", c.name, c.repo),
        CheckKind::Reading => format!("{} meets {} over {}", c.signal, c.condition, c.window),
        CheckKind::Submit => "the owner's review".to_owned(),
        CheckKind::Unverifiable => "nothing can confirm it".to_owned(),
    }
}

impl Shipping {
    /// state_check: the Chair declares how the result of the key result its chain serves will be checked (ADR-0068), for a
    /// direct request's key result or one that refines another and has no check yet (ADR-0115); the goal judge rules on
    /// it with the result. The check is one that can fail: a command run in a repository's worktree, or a reading of a
    /// signal, dry-run as an objective's measure is (ADR-0045); submit, the owner's eyes, only with the reason no
    /// executable check can exist, and that reason is a decision the owner reads. The key results the owner approved are
    /// his, and a refined one's check, once stated, is changed by whoever answers for it with refine_key_result.
    pub async fn state_check(&self, chair: &Card, c: Check) -> Result<String> {
        if chair.origin != Origin::Chair {
            return Err(Error::refused("only the Chair states the check"));
        }
        let c = crew::objective::trimmed_check(c);
        if c.kind == CheckKind::Unverifiable {
            return Err(Error::refused(
                "a direct request is checked: a command that can fail, a reading of a signal, or submit with the reason no \
                 executable check can exist",
            ));
        }
        c.valid().map_err(|e| Error::refused(e.to_string()))?;
        let home = chair.home.clone();
        let ch = self.store.read(move |r| r.chain(&home)).await?;
        let Some(serves) = ch.serves.clone().filter(|_| !verifying(&ch)) else {
            return Err(Error::refused(
                "this chain has no check to state: a verification chain has no judge",
            ));
        };
        let (oid, kid) = (serves.objective.clone(), serves.key_result.clone());
        let (obj, kr) = self
            .store
            .read(move |r| Ok::<_, Error>((r.objective(&oid)?, optional(r.key_result(&oid, &kid))?)))
            .await?;
        let Some(mut kr) = kr else {
            return Err(Error::refused(format!(
                "the objective has no key result {}",
                serves.key_result
            )));
        };
        if kr.parent.is_none() && !obj.task {
            return Err(Error::refused(
                "this chain serves a key result of an active objective: its check is the objective's, and a change to it \
                 is a new draft, applied in the owner's inbox",
            ));
        }
        if kr.parent.is_some() && kr.check.as_ref().is_some_and(|had| *had != c) {
            return Err(Error::refused(
                "this key result has its check: whoever answers for it changes it with refine_key_result",
            ));
        }
        kr.check = Some(c.clone());
        if c.kind != CheckKind::Submit {
            // a submit with its reason is the one check that is no measure, and is allowed here
            let draft = Objective {
                key_results: vec![kr.clone()],
                ..obj
            };
            let now = self.now();
            let found = self
                .store
                .read(move |r| crew::objective::checked_objective(r, &draft, now))
                .await?;
            let lines = finding_lines(
                &found
                    .into_iter()
                    .filter(|f| f.key_result == serves.key_result)
                    .collect::<Vec<Finding>>(),
            );
            if !lines.is_empty() {
                return Err(Error::refused(format!(
                    "the check is not stored, it cannot be run as stated:\n- {}",
                    lines.join("\n- ")
                )));
            }
        }
        let v = FactValue {
            chain: ch.id.clone(),
            key_result: serves.key_result.clone(),
            ..FactValue::default()
        };
        let fact = fact_write(keys::check_stated(), &v, by_agent(&chair.id), &ch.id)?;
        let door = self.door();
        let (w, now, stated) = (
            Authorship {
                by: by_agent(&chair.id),
                approved_by: None,
            },
            self.now(),
            c.clone(),
        );
        self.store
            .update(move |tx| {
                // read again where it is written: a check stated meanwhile on a refined key result stays
                let mut cur = tx.key_result(&serves.objective, &serves.key_result)?;
                if cur.parent.is_none() {
                    cur.check = Some(stated);
                    tx.put_key_result(&serves.objective, cur)?;
                } else if cur.check.as_ref().is_none_or(|had| *had == stated) {
                    // a refined key result's check is a change the owner reads, as refine_key_result's are
                    cur.check = Some(stated);
                    write_key_result(tx, &w, now, &serves.objective, cur)?;
                } else {
                    return Err(Error::refused(
                        "this key result has its check: whoever answers for it changes it with refine_key_result",
                    ));
                }
                door.ingest(tx, fact)?;
                Ok::<_, Error>(())
            })
            .await
            .map_err(|e| Error::refused(format!("the check was not stored: {e}")))?;
        let (text, why) = check_decision(&c);
        self.decide(&ch.id, DecisionKind::Goal, &text, &why, &chair.id, by_agent(&chair.id))
            .await;
        if !self.cfg.goal_check {
            return Ok("Recorded: the check.".to_owned());
        }
        Ok("Recorded: the check. When you finish, the goal judge rules on it with the result: a check that would pass \
            work that does not meet the key result is sent back to you to state again."
            .to_owned())
    }
}
