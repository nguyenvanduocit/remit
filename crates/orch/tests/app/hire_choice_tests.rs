//! The model of a hire (ADR-0118, point 1), driven as the Chair drives it: the cheapest tier a decider is sure of runs a
//! hire the Chair names no model for, a model the Chair names is never overridden, a gate signer, a hand-over and a
//! workflow's hire are untouched, and a card whose model code chose climbs a tier by Remit when its turn breaks or its
//! gate sends its change back twice. Only the gate scores the choice wrong; an accepted card scores it right.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use capability::accept::AcceptInput;
use chains::handover::HandOverInput;
use chains::hire_input::{HireInput, HireInputRef};
use chains::inbox::{Cause, Failed};
use judgment::{Decided, Decider, DeciderError, Question, Shape};
use model::{Card, DecisionKind, Edge, EdgeKind, EmployeeStatus, JudgmentAnswer, Signs};
use serde_json::Value;
use shipping::gate::VerdictInput;

use crate::shipping_land::{Landed, chain_seeded};
use crate::tests::{Rig, rig_of};

/// A decider that is sure a workhorse or frontier model can do the work and a fast one cannot, whatever the brief says,
/// and that the work wants medium effort. It counts its calls.
#[derive(Default)]
struct Frugal {
    calls: AtomicUsize,
}

#[async_trait]
impl Decider for Frugal {
    async fn decide(&self, _: &str, _: &Value, questions: &[Question]) -> Result<Decided, DeciderError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let answer = |q: &Question| match q.kind().shape() {
            Shape::Noul { yes, .. } => {
                let can = yes.starts_with("A workhorse") || yes.starts_with("A frontier");
                Some(JudgmentAnswer::Noul {
                    p: if can { 0.97 } else { 0.05 },
                })
            }
            Shape::Choice { options } => {
                let probabilities = options
                    .iter()
                    .map(|o| (o.name.clone(), if o.name == "medium" { 0.97 } else { 0.01 }))
                    .collect();
                Some(JudgmentAnswer::Choice {
                    choice: "medium".to_owned(),
                    probabilities,
                    confidence: 0.97,
                })
            }
            Shape::Score { .. } => None,
        };
        Ok(Decided {
            model: "frugal-1".to_owned(),
            answers: questions.iter().map(answer).collect(),
            cost_usd: 0.0,
        })
    }
}

/// A decider that answers after the time a point waits for one.
struct Late;

#[async_trait]
impl Decider for Late {
    async fn decide(&self, _: &str, _: &Value, _: &[Question]) -> Result<Decided, DeciderError> {
        tokio::time::sleep(Duration::from_secs(4)).await;
        Err(DeciderError::Failed("too late".to_owned()))
    }
}

fn configure(cfg: &mut orch::Config, decider: Option<Arc<dyn Decider>>) {
    cfg.core.models = Some(provider::Catalog::default());
    cfg.core.chair_model = "fable".to_owned();
    cfg.core.judge_model = "fable".to_owned();
    cfg.core.decider = decider;
}

fn rig_with(decider: Option<Arc<dyn Decider>>) -> Rig {
    rig_of(|cfg| configure(cfg, decider))
}

async fn chair_of(r: &Rig) -> Card {
    let chain = r.create("rq_chain", "Summarise the week").await;
    let id = chain.chair_id.clone();
    r.o.store.read(move |s| s.card(&id)).await.unwrap()
}

fn research(model: &str) -> HireInput {
    HireInput {
        agent: "research".to_owned(),
        brief: "Read the support tickets of the week and summarise the themes.".to_owned(),
        why: "the owner wants a weekly summary".to_owned(),
        deliverable: "a list of themes with counts".to_owned(),
        model: model.to_owned(),
        ..HireInput::default()
    }
}

async fn card(r: &Rig, id: &str) -> Card {
    let id = id.to_owned();
    r.o.store.read(move |s| s.card(&id)).await.unwrap()
}

/// The card that took over from old in its chain, once it exists; a test fails after 10 s.
async fn raised_from(r: &Rig, old: &Card, chair: &Card) -> Card {
    for _ in 0..200 {
        let home = old.home.clone();
        let cards = r.o.store.read(move |s| s.cards(&home)).await.unwrap();
        if let Some(c) = cards
            .into_iter()
            .find(|c| c.id != old.id && c.id != chair.id && c.employee == old.employee)
        {
            return c;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("no card took over from {}", old.id)
}

/// How many acted-on judgments of the tier question were proved wrong and unknown, as the owner's view counts them.
async fn tier_outcomes(r: &Rig) -> (i64, i64) {
    let key = judgment::HIRE_TIER_CAN.key();
    let tallies = r.o.store.read(|s| s.judgment_tallies()).await.unwrap();
    tallies
        .iter()
        .filter(|t| t.question == key && t.decider == judgment::DEFAULT_DECIDER)
        .fold((0, 0), |(wrong, unknown), t| {
            (wrong.saturating_add(t.wrong), unknown.saturating_add(t.unknown))
        })
}

async fn accept(r: &Rig, chair: &Card, card: &Card) {
    let accept = AcceptInput {
        card_id: card.id.clone(),
        verdict: model::ReportVerdict::Accept,
        notes: "the themes match the tickets".to_owned(),
        feedback: Vec::new(),
    };
    r.o.capabilities.accept_work(chair, accept).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hire_nobody_named_a_model_for_runs_on_the_cheapest_tier_a_decider_is_sure_of_and_says_who() {
    let r = rig_with(Some(Arc::new(Frugal::default())));
    let chair = chair_of(&r).await;
    let hired = r.o.hire(&chair, research("")).await.unwrap();
    assert_eq!(
        hired.model, "sonnet",
        "fast said no, workhorse yes, frontier yes: the cheapest yes"
    );
    assert_eq!(hired.effort, chair.effort, "the effort is only suggested, never used");
    assert!(hired.chosen.as_ref().is_some_and(|c| c.judgment.is_some()));
    let home = hired.home.clone();
    let said = r.o.store.read(move |s| s.decisions(&home)).await.unwrap();
    let hire = said.iter().find(|d| d.kind == DecisionKind::Hire).unwrap();
    assert!(
        hire.text.contains("model chosen by Jev 1.14, 97% sure")
            && hire.text.contains("effort left as set; Jev 1.14 suggests medium"),
        "{}",
        hire.text
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_model_the_chair_names_wins_and_never_climbs() {
    let r = rig_with(Some(Arc::new(Frugal::default())));
    let chair = chair_of(&r).await;
    let hired = r.o.hire(&chair, research("opus")).await.unwrap();
    assert_eq!(hired.model, "opus");
    assert!(hired.chosen.is_none(), "the Chair chose the model");
    r.o.core
        .climb_card(&hired.id, "it failed", model::ClimbCause::Failed)
        .await
        .unwrap();
    assert!(
        card(&r, &hired.id).await.handover.is_none(),
        "no climb: the model was not code's"
    );
    let home = hired.home.clone();
    assert_eq!(
        r.o.store.read(move |s| s.cards(&home)).await.unwrap().len(),
        2,
        "the Chair and the hire only"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn no_decider_or_a_late_one_leaves_the_choice_to_the_chair_as_it_was() {
    for decider in [None::<Arc<dyn Decider>>, Some(Arc::new(Late))] {
        let r = rig_with(decider);
        let chair = chair_of(&r).await;
        let hired = r.o.hire(&chair, research("")).await.unwrap();
        assert_eq!(hired.model, "fable", "the Chair's own model");
        assert!(hired.chosen.is_none());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_gate_signer_runs_on_the_judges_model_and_no_decider_is_asked() {
    let decider = Arc::new(Frugal::default());
    let r = rig_with(Some(decider.clone()));
    let chair = chair_of(&r).await;
    let author = r.o.hire(&chair, research("")).await.unwrap();
    let asked = decider.calls.load(Ordering::SeqCst);
    let review = HireInput {
        employee: author.employee.clone(),
        role: "reviewer".to_owned(),
        brief: "Review the summary.".to_owned(),
        why: "a second pair of eyes".to_owned(),
        inputs: vec![HireInputRef {
            card_id: author.id.clone(),
            label: "the summary".to_owned(),
        }],
        ..HireInput::default()
    };
    let judge = r.o.hire(&chair, review).await.unwrap();
    assert_eq!(judge.signs, Signs::Review);
    assert_eq!(judge.model, "fable");
    assert!(judge.chosen.is_none());
    assert_eq!(
        decider.calls.load(Ordering::SeqCst),
        asked,
        "a gate is a rail: nothing is asked for it"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_chairs_hand_over_of_a_chosen_card_keeps_its_model_and_asks_no_decider() {
    let decider = Arc::new(Frugal::default());
    let r = rig_with(Some(decider.clone()));
    let chair = chair_of(&r).await;
    let old = r.o.hire(&chair, research("")).await.unwrap();
    let asked = decider.calls.load(Ordering::SeqCst);
    let input = HandOverInput {
        card_id: old.id.clone(),
        employee: old.employee.clone(),
        why: "it keeps missing the counts".to_owned(),
        ..HandOverInput::default()
    };
    r.o.core.hand_over_card(&chair, input).await.unwrap();
    r.o.core.complete_handover(&old.id, "the note", "core").await.unwrap();
    let new = raised_from(&r, &old, &chair).await;
    assert_eq!(new.model, "sonnet", "never below the card it takes over from");
    assert!(new.chosen.is_some(), "and still the layer's choice, so it can climb");
    assert_eq!(
        decider.calls.load(Ordering::SeqCst),
        asked,
        "a hand-over asks no decider"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_worker_whose_turn_breaks_is_raised_a_tier_by_remit_but_the_choice_is_not_scored_wrong() {
    let r = rig_with(Some(Arc::new(Frugal::default())));
    let chair = chair_of(&r).await;
    let old = r.o.hire(&chair, research("")).await.unwrap();
    r.o.core
        .respond_to_failed_turn(&old, &Failed::new(Cause::Harness, "Claude Code ended its turn"), None)
        .await;
    let raised = raised_from(&r, &old, &chair).await;
    assert_eq!(
        (raised.model.as_str(), raised.effort.as_str()),
        ("opus", old.effort.as_str())
    );
    assert_eq!(raised.chosen, Some(model::Chosen::default()));
    let left = card(&r, &old.id).await;
    assert!(left.activity.starts_with("handed over"), "{}", left.activity);
    assert!(left.chosen.is_none());
    let home = old.home.clone();
    let said = r.o.store.read(move |s| s.decisions(&home)).await.unwrap();
    let remit = |d: &&model::Decision| d.by.as_ref().is_some_and(|b| b.kind == model::ActorKind::Remit);
    assert!(
        said.iter().filter(remit).any(|d| d.kind == DecisionKind::Handover
            && d.text.starts_with("Raise ")
            && d.why.contains("Claude Code ended")),
        "{said:?}"
    );
    assert!(
        said.iter()
            .filter(remit)
            .any(|d| d.kind == DecisionKind::Hire && d.card_id == raised.id),
        "the raised card's hire is Remit's decision: {said:?}"
    );
    assert_eq!(
        tier_outcomes(&r).await,
        (0, 1),
        "a broken turn says little of the model: unknown, in the hand-over's own change"
    );
    let open = r.o.store.read(|s| s.open_inbox()).await.unwrap();
    assert!(
        open.iter()
            .all(|i| !(i.card_id == old.id && i.detail.contains("Claude Code ended"))),
        "no Retry: Remit took the card: {open:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failure_a_stronger_model_would_not_change_is_reported_and_the_card_stays() {
    let r = rig_with(Some(Arc::new(Frugal::default())));
    let chair = chair_of(&r).await;
    let old = r.o.hire(&chair, research("")).await.unwrap();
    r.o.core
        .respond_to_failed_turn(&old, &Failed::new(Cause::NotOpened, "no session"), None)
        .await;
    assert!(card(&r, &old.id).await.handover.is_none());
    let open = r.o.store.read(|s| s.open_inbox()).await.unwrap();
    assert_eq!(
        open.iter()
            .filter(|i| i.card_id == old.id && i.detail.contains("no session"))
            .count(),
        1,
        "the owner's Assistant is told: {open:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_climb_the_hiring_rules_refuse_leaves_the_failure_to_the_assistant_as_it_was() {
    let r = rig_with(Some(Arc::new(Frugal::default())));
    let chair = chair_of(&r).await;
    let old = r.o.hire(&chair, research("")).await.unwrap();
    let employee = old.employee.clone();
    r.o.store
        .update(move |tx| {
            let mut e = tx.employee(&employee)?;
            e.status = EmployeeStatus::Retired;
            tx.put_employee(e)
        })
        .await
        .unwrap();
    r.o.core
        .respond_to_failed_turn(&old, &Failed::new(Cause::Harness, "Claude Code ended its turn"), None)
        .await;
    let mut told = 0;
    for _ in 0..100 {
        let open = r.o.store.read(|s| s.open_inbox()).await.unwrap();
        told = open
            .iter()
            .filter(|i| i.card_id == old.id && i.detail.contains("Claude Code ended"))
            .count();
        if told > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(told, 1, "the Assistant gets the Retry item");
    let left = card(&r, &old.id).await;
    assert!(left.handover.is_none() && !left.activity.starts_with("handed over"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_card_the_chair_accepts_proved_its_choice_right() {
    let r = rig_with(Some(Arc::new(Frugal::default())));
    let chair = chair_of(&r).await;
    let hired = r.o.hire(&chair, research("")).await.unwrap();
    accept(&r, &chair, &hired).await;
    let scores = r.o.store.read(|s| s.judgment_scores(&[])).await.unwrap();
    let acting = scores
        .iter()
        .filter(|s| s.right && s.decider == judgment::DEFAULT_DECIDER);
    assert_eq!(acting.count(), 1, "{scores:?}");
}

/// A chain with one worktree and a reviewer that judges `authors` hired workers there, each accepted, the change landed
/// and judging.
struct Gate {
    t: Landed,
    reviewer: Card,
    authors: Vec<Card>,
    worktree: String,
}

async fn gate_of(authors: usize) -> Gate {
    let t = chain_seeded(
        |_| {},
        |cfg| configure(cfg, Some(Arc::new(Frugal::default()))),
        |wt| std::fs::write(wt.join("hello.txt"), "hello\n").unwrap(),
    )
    .await;
    let r = &t.r;
    let id = t.chain.id.clone();
    let worktree =
        r.o.store
            .read(move |s| s.chain(&id))
            .await
            .unwrap()
            .repos
            .first()
            .unwrap()
            .path
            .clone();
    let mut hired: Vec<Card> = Vec::new();
    for _ in 0..authors {
        let mut hire = research("");
        hire.cwd.clone_from(&worktree);
        // The Chair reuses idle workers before it hires: whether an earlier author is idle yet depends on when its
        // session's start failed in this rig, so the hire says, as the rule asks, which it does not choose and why.
        for earlier in &hired {
            hire.why = format!(
                "{} Not {}: it is the other author of this change, and a change made by two is what is tested.",
                hire.why, earlier.employee
            );
        }
        let author = r.o.hire(&t.chair, hire).await.unwrap();
        accept(r, &t.chair, &author).await;
        hired.push(author);
    }
    t.land().await;
    let mut reviewer = Card::default();
    reviewer.id = "c_reviewer".to_owned();
    reviewer.home.clone_from(&t.chain.id);
    reviewer.name = "Reviewer".to_owned();
    reviewer.signs = Signs::Review;
    let edges: Vec<Edge> = hired
        .iter()
        .map(|a| Edge {
            id: format!("e_review_{}", a.id),
            chain_id: t.chain.id.clone(),
            from: a.id.clone(),
            to: reviewer.id.clone(),
            kind: EdgeKind::Handoff,
            label: "the change".to_owned(),
            ..Edge::default()
        })
        .collect();
    let reviewer =
        r.o.store
            .update(move |tx| {
                for e in &edges {
                    tx.put_edge(e)?;
                }
                tx.put_card(reviewer)
            })
            .await
            .unwrap();
    Gate {
        t,
        reviewer,
        authors: hired,
        worktree,
    }
}

impl Gate {
    /// The reviewer's request for changes on the change judged now, as its tool records it.
    async fn request_changes(&self) {
        let input: VerdictInput = serde_json::from_value(serde_json::json!({
            "verdict": "request_changes",
            "notes": "the greeting is missing its punctuation",
            "findings": [{"keyResult": "", "item": "the greeting", "status": "not_met", "evidence": "hello.txt:1", "code": "other"}],
        }))
        .unwrap();
        self.t.r.o.shipping.record_verdict(&self.reviewer, input).await.unwrap();
    }

    /// The authors amend and land a second version, which the reviewer is to judge.
    async fn amend(&self) {
        let dir = std::path::Path::new(&self.worktree);
        std::fs::write(dir.join("hello.txt"), "hello, world\n").unwrap();
        crate::shipping_land::sh(dir, &["commit", "-qam", "punctuate"]);
        self.t.land().await;
        self.t
            .until(|l| l.version.number == 2, |l| l.state == model::LandingState::Judging)
            .await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_author_of_a_change_its_gate_sent_back_twice_is_raised_once_and_the_choice_scored_wrong() {
    let g = gate_of(1).await;
    let (r, chair, author) = (&g.t.r, &g.t.chair, g.authors.first().unwrap());
    // the same change asked changes to twice (a retry) is one round
    g.request_changes().await;
    g.request_changes().await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(card(r, &author.id).await.handover.is_none(), "one round is no climb");

    g.amend().await;
    g.request_changes().await;
    let raised = raised_from(r, author, chair).await;
    assert_eq!(raised.model, "opus");
    assert_eq!(raised.employee, author.employee, "the same employee goes on, a tier up");
    assert_eq!(tier_outcomes(r).await, (1, 0), "the gate proved the tier too low");
    let home = raised.home.clone();
    let edges = r.o.store.read(move |s| s.edges(&home)).await.unwrap();
    assert!(
        edges.iter().any(|e| e.from == raised.id && e.to == g.reviewer.id),
        "the reviewer judges the raised card too, so its own rounds count: {edges:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_change_two_chosen_authors_made_is_raised_for_neither() {
    let g = gate_of(2).await;
    g.request_changes().await;
    g.amend().await;
    g.request_changes().await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    for a in &g.authors {
        assert!(
            card(&g.t.r, &a.id).await.handover.is_none(),
            "the weak model cannot be told from the change"
        );
    }
    assert_eq!(tier_outcomes(&g.t.r).await, (0, 0));
}
