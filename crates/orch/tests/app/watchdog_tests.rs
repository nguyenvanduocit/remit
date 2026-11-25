//! The watchdog on a running card (ADR-0118, point 6), driven as a worker's report drives it: when a decider is sure the
//! turn drifted, the Chair reads a note with the report; unsure, late or silent, it reads the report as it always did. The
//! Chair's verdict then scores both questions both ways, and nothing is ever paused, stopped or refused.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use capability::accept::AcceptInput;
use chains::apply::Records;
use chains::hire_input::HireInput;
use chains::turn_end::WorkerReport;
use judgment::{CARD_OFF_BRIEF, CARD_PROGRESS, Decided, Decider, DeciderError, Question};
use model::{
    Card, Judgment, JudgmentAnswer, JudgmentOutcome, JudgmentPick, JudgmentSubject, JudgmentUse, OutcomeSource, Proved,
    ReportVerdict,
};
use serde_json::Value;

use crate::tests::{Rig, rig_of};

/// A decider that reads a turn as stuck and off its brief with the probabilities it is given (of yes), and gives no
/// answer to any other question.
struct Reading {
    progress: f64,
    off_brief: f64,
}

#[async_trait]
impl Decider for Reading {
    async fn decide(&self, _: &str, _: &Value, questions: &[Question]) -> Result<Decided, DeciderError> {
        let answer = |q: &Question| {
            let p = match q.key() {
                k if k == CARD_PROGRESS.key() => self.progress,
                k if k == CARD_OFF_BRIEF.key() => self.off_brief,
                _ => return None,
            };
            Some(JudgmentAnswer::Noul { p })
        };
        Ok(Decided {
            model: "reading-1".to_owned(),
            answers: questions.iter().map(answer).collect(),
            cost_usd: 0.0,
        })
    }
}

struct Late;

#[async_trait]
impl Decider for Late {
    async fn decide(&self, _: &str, _: &Value, _: &[Question]) -> Result<Decided, DeciderError> {
        tokio::time::sleep(Duration::from_secs(4)).await;
        Err(DeciderError::Failed("too late".to_owned()))
    }
}

/// A rig whose decider is decider; a chain with its Chair, and a worker hired on a model the Chair names (so only the
/// watchdog asks a decider).
async fn rig_with(decider: Option<Arc<dyn Decider>>) -> (Rig, Card, Card) {
    let r = rig_of(|cfg| {
        cfg.core.models = Some(provider::Catalog::default());
        cfg.core.chair_model = "fable".to_owned();
        cfg.core.judge_model = "fable".to_owned();
        cfg.core.decider = decider;
    });
    let chain = r.create("rq_watch", "Tidy the logging").await;
    let id = chain.chair_id.clone();
    let chair = r.o.store.read(move |s| s.card(&id)).await.unwrap();
    let worker =
        r.o.hire(
            &chair,
            HireInput {
                agent: "research".to_owned(),
                brief: "Replace the print calls of the logging module with the logger.".to_owned(),
                why: "the owner wants one logger".to_owned(),
                deliverable: "a module that logs through the logger alone".to_owned(),
                model: "opus".to_owned(),
                ..HireInput::default()
            },
        )
        .await
        .unwrap();
    // The hire opens the worker's session, which fails in these tests and reports each failure to the Chair, and every
    // report is the card's last until the next: wait until they stop, so the test's own report is the last.
    let (mut seen, mut quiet) = (0_usize, 0_u32);
    for _ in 0..200 {
        let id = chair.id.clone();
        let events = r.o.store.read(move |s| s.card_events(&id, 200)).await.unwrap();
        let now = events.iter().filter(|e| e.text.starts_with("[report from")).count();
        quiet = if now == seen { quiet.saturating_add(1) } else { 0 };
        seen = now;
        if seen > 0 && quiet >= 10 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
    (r, chair, worker)
}

/// The worker reports its turn to its Chair; what the Chair is sent, the newest message.
async fn report(r: &Rig, chair: &Card, worker: &Card, said: &str) -> String {
    let text = said.to_owned();
    let w = WorkerReport {
        output: text.clone(),
        stop: "end_turn".to_owned(),
        by: model::Actor::default(),
    };
    r.o.core
        .send_report(worker, w, move |c| c.output.clone_from(&text), Records::default())
        .await
        .unwrap();
    let chair = chair.id.clone();
    let events = r.o.store.read(move |s| s.card_events(&chair, 100)).await.unwrap();
    events
        .into_iter()
        .rfind(|e| e.text.starts_with("[report from") && e.text.contains(said))
        .map(|e| e.text)
        .unwrap_or_default()
}

async fn rule(r: &Rig, chair: &Card, worker: &Card, verdict: ReportVerdict) {
    let input = AcceptInput {
        card_id: worker.id.clone(),
        verdict,
        notes: "checked against the criteria".to_owned(),
        feedback: vec!["use the logger in every module".to_owned()],
    };
    r.o.capabilities.accept_work(chair, input).await.unwrap();
}

/// How many of a watchdog question's acting answers proved right, wrong and unknown.
async fn outcomes(r: &Rig, question: &str) -> (i64, i64, i64) {
    let tallies = r.o.store.read(|s| s.judgment_tallies()).await.unwrap();
    tallies
        .iter()
        .filter(|t| t.question == question && t.decider == judgment::DEFAULT_DECIDER)
        .fold((0, 0, 0), |(right, wrong, unknown), t| {
            (
                right.saturating_add(t.right),
                wrong.saturating_add(t.wrong),
                unknown.saturating_add(t.unknown),
            )
        })
}

/// The watchdog's answers about the worker's reports that no verdict has ruled on yet.
async fn open_answers(r: &Rig, worker: &Card) -> Vec<Judgment> {
    let id = worker.id.clone();
    r.o.store
        .read(move |s| s.open_report_judgments(&id, &[CARD_PROGRESS.key(), CARD_OFF_BRIEF.key()]))
        .await
        .unwrap()
}

const DRIFT: &str = "I read the module again and listed what I could change.";

#[tokio::test(flavor = "multi_thread")]
async fn a_report_a_decider_is_sure_drifts_reaches_the_chair_with_a_note_and_an_accept_scores_both_answers_wrong() {
    let (r, chair, worker) = rig_with(Some(Arc::new(Reading {
        progress: 0.03,
        off_brief: 0.96,
    })))
    .await;
    let sent = report(&r, &chair, &worker, DRIFT).await;
    assert!(
        sent.contains("[watchdog \u{b7} advice, not a verdict]")
            && sent.contains("moving the card no closer to its brief")
            && sent.contains("work its brief did not ask for")
            && sent.contains("97% sure")
            && sent.contains(DRIFT),
        "the Chair reads the note with the report: {sent}"
    );
    let kept = open_answers(&r, &worker).await;
    assert!(
        kept.len() == 2
            && kept.iter().all(|j| j.used == JudgmentUse::Informed
                && j.threshold == Some(judgment::PROVISIONAL_THRESHOLD)
                && j.subject.report_of().is_some_and(|(card, _)| card == worker.id)),
        "each answer is kept with the report it is about, the bar it was held to and that it was shown: {kept:?}"
    );
    rule(&r, &chair, &worker, ReportVerdict::Accept).await;
    // An accepted report was moving toward its brief and on it: both alarms were wrong.
    assert_eq!(outcomes(&r, &CARD_PROGRESS.key()).await, (0, 1, 0));
    assert_eq!(outcomes(&r, &CARD_OFF_BRIEF.key()).await, (0, 1, 0));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_report_sent_back_for_rework_scores_both_alarms_right() {
    let (r, chair, worker) = rig_with(Some(Arc::new(Reading {
        progress: 0.03,
        off_brief: 0.96,
    })))
    .await;
    report(&r, &chair, &worker, DRIFT).await;
    rule(&r, &chair, &worker, ReportVerdict::Rework).await;
    assert_eq!(outcomes(&r, &CARD_PROGRESS.key()).await, (1, 0, 0));
    assert_eq!(outcomes(&r, &CARD_OFF_BRIEF.key()).await, (1, 0, 0));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_decider_that_reads_the_turn_as_fine_is_scored_by_the_same_verdict_and_says_nothing() {
    let (r, chair, worker) = rig_with(Some(Arc::new(Reading {
        progress: 0.97,
        off_brief: 0.02,
    })))
    .await;
    let sent = report(&r, &chair, &worker, "Replaced every print call; the tests pass.").await;
    assert!(!sent.contains("[watchdog"), "{sent}");
    let kept = open_answers(&r, &worker).await;
    assert!(
        kept.len() == 2 && kept.iter().all(|j| j.used == JudgmentUse::Held),
        "an answer that said nothing is kept as held, and is scored all the same: {kept:?}"
    );
    rule(&r, &chair, &worker, ReportVerdict::Rework).await;
    // It said the card was moving and on its brief, and the Chair sent it back: wrong both ways.
    assert_eq!(outcomes(&r, &CARD_PROGRESS.key()).await, (0, 1, 0));
    assert_eq!(outcomes(&r, &CARD_OFF_BRIEF.key()).await, (0, 1, 0));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unsure_late_or_missing_decider_leaves_the_report_as_it_always_was() {
    let unsure: Arc<dyn Decider> = Arc::new(Reading {
        progress: 0.4,
        off_brief: 0.6,
    });
    let late: Arc<dyn Decider> = Arc::new(Late);
    for decider in [Some(unsure), Some(late), None] {
        let (r, chair, worker) = rig_with(decider).await;
        let sent = report(&r, &chair, &worker, DRIFT).await;
        assert!(
            sent.starts_with("[report from") && !sent.contains("[watchdog"),
            "{sent}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_report_overtaken_by_another_before_any_verdict_proves_nothing() {
    let (r, chair, worker) = rig_with(Some(Arc::new(Reading {
        progress: 0.03,
        off_brief: 0.96,
    })))
    .await;
    report(&r, &chair, &worker, DRIFT).await;
    report(&r, &chair, &worker, "Now I replaced the print calls.").await;
    rule(&r, &chair, &worker, ReportVerdict::Rework).await;
    assert_eq!(
        outcomes(&r, &CARD_PROGRESS.key()).await,
        (1, 0, 1),
        "the verdict is of the newest report; the first was never ruled on"
    );
}

/// Thirty answers of `decider` to the progress question that proved right: enough for it to be measured, and (being surer
/// than the default, which has none) to rank first.
async fn measured(r: &Rig, decider: &str) {
    let judgments: Vec<Judgment> = (0..judgment::MIN_OUTCOMES)
        .map(|i| Judgment {
            id: format!("ju_seed{i}"),
            asking: format!("ja_seed{i}"),
            question: CARD_PROGRESS.key(),
            subject: JudgmentSubject::report("c_seed", &format!("rp_seed{i}"), "ch_seed"),
            decider: decider.to_owned(),
            answer: JudgmentAnswer::Noul { p: 0.99 },
            pick: JudgmentPick::Yes,
            p: 0.99,
            used: JudgmentUse::Informed,
            outcome: Some(JudgmentOutcome {
                proved: Proved::Right,
                source: OutcomeSource::Card,
                at: model::Time::default(),
            }),
            ..Judgment::default()
        })
        .collect();
    r.o.store.update(move |tx| tx.put_judgments(judgments)).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_report_waits_for_the_watchdog_once_however_many_deciders_are_tried() {
    let late: Arc<dyn Decider> = Arc::new(Late);
    let (r, chair, worker) = rig_with(Some(late)).await;
    // A measured decider ranks first and the default after it: two rounds of asking, each 2 s when the decider is late.
    measured(&r, "liquid/d1.2-ultra").await;
    let started = std::time::Instant::now();
    let sent = report(&r, &chair, &worker, DRIFT).await;
    let waited = started.elapsed();
    assert!(
        sent.starts_with("[report from") && !sent.contains("[watchdog"),
        "{sent}"
    );
    assert!(
        waited < Duration::from_millis(3500),
        "one deadline of {:?} around the whole asking, not one a round: waited {waited:?}",
        chains::judge::DECIDE_FOR
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_answer_whose_note_was_never_sent_is_not_recorded_as_shown() {
    let (r, _chair, worker) = rig_with(Some(Arc::new(Reading {
        progress: 0.03,
        off_brief: 0.96,
    })))
    .await;
    let said = r.o.core.watch_report(&worker, "rp_unsent", DRIFT).await;
    assert!(
        said.is_some_and(|w| w.note.contains("[watchdog")),
        "the deciders did answer"
    );
    let kept = open_answers(&r, &worker).await;
    assert!(
        kept.len() == 2 && kept.iter().all(|j| j.used == JudgmentUse::Held),
        "no report carried the note, so nothing was shown: {kept:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_turn_sent_back_leaves_the_answers_about_the_report_before_it_without_an_outcome() {
    let (r, chair, worker) = rig_with(Some(Arc::new(Reading {
        progress: 0.03,
        off_brief: 0.96,
    })))
    .await;
    report(&r, &chair, &worker, DRIFT).await;
    let failed = WorkerReport {
        output: "Agent failed: the session closed".to_owned(),
        stop: "failed".to_owned(),
        by: model::Actor::default(),
    };
    r.o.core
        .send_report(&worker, failed, |_| {}, Records::default())
        .await
        .unwrap();
    rule(&r, &chair, &worker, ReportVerdict::Rework).await;
    assert_eq!(
        outcomes(&r, &CARD_PROGRESS.key()).await,
        (0, 0, 1),
        "the verdict is of the failure, which nothing proved"
    );
}
