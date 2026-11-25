//! The watchdog on a running card (ADR-0118, point 6). When a worker's report is about to go to its Chair, two typed
//! questions are asked of it against the card's brief: `card.progress` (did the turn move the card toward its brief?) and
//! `card.off_brief` (did it do work the brief did not ask for?). It catches the drift that the repeat and spin
//! fingerprints (`repeats.rs`, `spin.rs`) cannot see, because it reads what the turn said, not how often. An answer that
//! reads the card as drifting and is at or above its bar (`judgment::use_of`: the question's act threshold, 0.9 until a
//! decider has 30 outcomes) is shown to the Chair as a line in the report's own message, so it reads it with the report; every
//! other answer is recorded as held. It is advice: nothing is paused, stopped or refused (those are rails), and with no
//! decider, a late one or a refusal the Chair is told nothing, as before. The report waits at most `judge::DECIDE_FOR` (2 s)
//! for all of it, one deadline around the whole asking, however many deciders are tried.
//!
//! Each answer is recorded on the report it is about (`JudgmentSubject::report`, the id kept on the card as
//! `Card::report`), with the bar it was held to and whether it was shown (`Informed` or `Held`), so a later reading of the
//! results can tell what the Chair saw. The outcome is the Chair's verdict on that report (`proved_by`, written in the
//! commit of the decision): an accepted report was moving toward its brief and on it, one sent back for rework was not.
//! Only the answers about the report the verdict rules on are scored; those about an earlier report the card replaced
//! before any verdict are left without an outcome. Both questions are scored both ways, so a decider that never raises an
//! alarm is wrong whenever a card is sent back. It is a proxy: a report can be sent back for a flaw that is not drift, and
//! the note itself makes a rework likelier, which is why whether it was shown is kept (*Revisit when*, ADR-0118).

use judgment::{CARD_OFF_BRIEF, CARD_PROGRESS, Kind, Question, decider_name};
use model::{
    Card, Decision, DecisionKind, Judgment, JudgmentOutcome, JudgmentPick, JudgmentSubject, JudgmentUse, OutcomeSource,
    Proved, Time,
};
use serde_json::json;
use store::Tx;

use crate::Result;
use crate::chain_core::Core;
use crate::hire_choice::sure;
use crate::judge::{DECIDE_FOR, Judged};
use crate::recap::clip;

/// How much of a report the deciders read: the newest words of a long one are its conclusion, which a recap keeps too.
const REPORT_RUNES: usize = 8000;

/// Whether a report that ended as stop says what a turn did: one that ended its turn or was written with the report tool,
/// not the failure of a session that did not open or a turn that was cut.
pub(crate) fn watched(stop: &str) -> bool {
    stop == model::StopReason::EndTurn.as_str() || stop == REPORT_STOP
}

/// How a report written with the report tool is stopped (`Core::report_now`).
pub(crate) const REPORT_STOP: &str = "report";

fn question(
    definition: &'static judgment::Definition,
    kind: std::result::Result<Kind, judgment::Invalid>,
) -> Option<Question> {
    kind.and_then(|kind| Question::new(definition, kind))
        .inspect_err(
            |err| tracing::warn!(%err, question = definition.key(), "watchdog: a question that cannot be asked"),
        )
        .ok()
}

fn progress_question() -> Option<Question> {
    question(
        &CARD_PROGRESS,
        Kind::noul(
            "The turn moved the card toward its brief: it did work the brief asks for, and its report says what is done.",
            "The turn did not move the card toward its brief: it repeated itself, stalled, or reports nothing done.",
        ),
    )
}

fn off_brief_question() -> Option<Question> {
    question(
        &CARD_OFF_BRIEF,
        Kind::noul(
            "The turn did work the brief did not ask for, or ignored what the brief asks for.",
            "The turn kept to the work the brief asks for.",
        ),
    )
}

/// The line that tells the Chair what the answer j, which was shown, says of its card. Pure.
fn voiced(j: &Judged, words: &str) -> String {
    format!(
        "{}, {}, reads this turn as {words}",
        decider_name(&j.decider),
        sure(j.p)
    )
}

/// The Chair's note for the lines that were voiced. Pure.
fn note_of(lines: &[String]) -> String {
    format!(
        "\n\n[watchdog · advice, not a verdict] {}. Remit does nothing else with it: read the report against the acceptance criteria before you accept it.",
        lines.join("; ")
    )
}

/// What the watchdog has to say of a report: the note, and the judgments it shows.
pub struct Watched {
    pub note: String,
    /// The ids of the judgments the note says, to be stored as shown with the report that carries it.
    pub shown: Vec<String>,
}

impl Core {
    /// What the deciders make of report, which card sends its Chair as the report `report_id`, against its brief: a note for
    /// the Chair when one is sure enough that the card is drifting. None when nothing is to be said, or there is no
    /// decider, it is late, or it refuses: the report goes as it always has, and waits at most `DECIDE_FOR` for it, the
    /// whole asking under one deadline.
    pub async fn watch_report(&self, card: &Card, report_id: &str, report: &str) -> Option<Watched> {
        if report.trim().is_empty() || card.brief.trim().is_empty() {
            return None;
        }
        let (progress, off_brief) = (progress_question()?, off_brief_question()?);
        let state = json!({
            "brief": card.brief,
            "deliverable": card.deliverable,
            "criteria": card.criteria,
            "plan": card.plan.iter().map(|p| format!("[{}] {}", p.status, p.content)).collect::<Vec<_>>(),
            "report": clip(report, REPORT_RUNES),
        });
        let subject = JudgmentSubject::report(&card.id, report_id, &card.home);
        let asked = tokio::time::timeout(DECIDE_FOR, self.judge(subject, &state, vec![progress, off_brief]))
            .await
            .inspect_err(|_| {
                tracing::info!(
                    card = card.id,
                    "watchdog: no answer in time; the report goes without it"
                )
            })
            .ok()?;
        let mut lines = Vec::new();
        let mut shown = Vec::new();
        for (answer, words) in [
            (asked.first(), "moving the card no closer to its brief"),
            (asked.get(1), "work its brief did not ask for"),
        ] {
            if let Some(j) = answer
                .and_then(Option::as_ref)
                .filter(|j| j.used == JudgmentUse::Informed)
            {
                lines.push(voiced(j, words));
                shown.push(j.judgment.clone());
            }
        }
        (!lines.is_empty()).then(|| Watched {
            note: note_of(&lines),
            shown,
        })
    }
}

/// What the Chair's verdict says of the pick of a watchdog question: an accepted report moved toward its brief and kept
/// to it, one sent back did not. A question that is neither is proved nothing. Pure.
fn proves(question: &str, pick: &JudgmentPick, accepted: bool) -> Proved {
    let says_accepted = match (question, pick) {
        (q, JudgmentPick::Yes) if q == CARD_PROGRESS.key() => true,
        (q, JudgmentPick::No) if q == CARD_PROGRESS.key() => false,
        (q, JudgmentPick::No) if q == CARD_OFF_BRIEF.key() => true,
        (q, JudgmentPick::Yes) if q == CARD_OFF_BRIEF.key() => false,
        _ => return Proved::Unknown,
    };
    if says_accepted == accepted {
        Proved::Right
    } else {
        Proved::Wrong
    }
}

/// Records in tx what the Chair's verdict d proves of the watchdog's answers about the report it judges, when d accepts a
/// card's report or sends it back: part of the verdict's own commit, so no verdict is recorded without it. The verdict
/// rules on the card's last report (`Card::report`): its answers are proved, and the answers about any earlier report, which
/// the card replaced before a verdict was given, are left without an outcome (`Proved::Unknown`).
pub(crate) fn proved_by(tx: &Tx<'_>, d: &Decision, at: Time) -> Result<()> {
    let accepted = match d.kind {
        DecisionKind::Accept => true,
        DecisionKind::Rework => false,
        _ => return Ok(()),
    };
    if d.card_id.is_empty() {
        return Ok(());
    }
    let ruled_on = tx.card(&d.card_id)?.report.unwrap_or_default();
    for j in tx.open_report_judgments(&d.card_id, &[CARD_PROGRESS.key(), CARD_OFF_BRIEF.key()])? {
        let proved = if j.subject.report_of().is_some_and(|(_, report)| report == ruled_on) {
            proves(&j.question, &j.pick, accepted)
        } else {
            Proved::Unknown
        };
        set(tx, &j, proved, at)?;
    }
    Ok(())
}

fn set(tx: &Tx<'_>, j: &Judgment, proved: Proved, at: Time) -> Result<()> {
    let outcome = JudgmentOutcome {
        proved,
        source: OutcomeSource::Card,
        at,
    };
    match tx.set_judgment_outcome(&j.id, &outcome) {
        Ok(()) | Err(store::Error::NotFound) => Ok(()),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::JudgmentAnswer;

    fn answered(pick: JudgmentPick, p: f64) -> Judged {
        Judged {
            judgment: "ju_1".to_owned(),
            decider: judgment::DEFAULT_DECIDER.to_owned(),
            answer: JudgmentAnswer::Noul { p },
            pick,
            p,
            used: model::JudgmentUse::Informed,
            threshold: Some(0.9),
        }
    }

    #[test]
    fn an_accepted_report_proves_progress_and_a_sent_back_one_proves_drift_each_way() {
        let (progress, off) = (CARD_PROGRESS.key(), CARD_OFF_BRIEF.key());
        assert_eq!(proves(&progress, &JudgmentPick::Yes, true), Proved::Right);
        assert_eq!(proves(&progress, &JudgmentPick::Yes, false), Proved::Wrong);
        assert_eq!(proves(&progress, &JudgmentPick::No, false), Proved::Right);
        assert_eq!(proves(&progress, &JudgmentPick::No, true), Proved::Wrong);
        assert_eq!(proves(&off, &JudgmentPick::Yes, false), Proved::Right);
        assert_eq!(proves(&off, &JudgmentPick::Yes, true), Proved::Wrong);
        assert_eq!(proves(&off, &JudgmentPick::No, true), Proved::Right);
        assert_eq!(proves(&off, &JudgmentPick::No, false), Proved::Wrong);
    }

    #[test]
    fn a_voiced_answer_says_who_and_how_sure() {
        let said = voiced(
            &answered(JudgmentPick::No, 0.95),
            "moving the card no closer to its brief",
        );
        assert!(said.contains("95% sure") && said.contains("Jev 1.14"), "{said}");
    }
}
