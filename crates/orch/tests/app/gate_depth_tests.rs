//! How deep a change's review goes (ADR-0118, point 4), driven as the Chair drives it: when the Chair lands a version, a
//! decider says how risky it is and whether it alters a rail; what it is sure of raises the effort of the change reviewers
//! the Chair hires and adds the security gate, and nothing it says lowers a gate. The landing's ending scores it: live is
//! it holding, a revert is it breaking.

use std::sync::Arc;

use async_trait::async_trait;
use chains::hire_input::{HireInput, HireInputRef};
use judgment::{Decided, Decider, DeciderError, Question, Shape};
use model::{Card, DecisionKind, Gate, JudgmentAnswer, Landing, LandingState, ReleaseState, Signs, by_owner};
use serde_json::Value;
use shipping::gate::VerdictInput;
use shipping::revert::RevertInput;

use crate::shipping_land::{Landed, chain_seeded};

/// A decider that finds every change as risky as `level` (0 the least) with probability `sure`, and says a change alters
/// a rail with probability `rail`.
struct Sure {
    level: usize,
    rail: f64,
    sure: f64,
}

#[async_trait]
impl Decider for Sure {
    async fn decide(&self, _: &str, _: &Value, questions: &[Question]) -> Result<Decided, DeciderError> {
        let answer = |q: &Question| match q.kind().shape() {
            Shape::Score { levels } => {
                let probabilities: Vec<f64> = (0..levels.len())
                    .map(|i| {
                        if i == self.level {
                            self.sure
                        } else {
                            (1.0 - self.sure) / 2.0
                        }
                    })
                    .collect();
                Some(JudgmentAnswer::Score {
                    score: 0.0,
                    probabilities,
                    confidence: self.sure,
                })
            }
            Shape::Noul { .. } => Some(JudgmentAnswer::Noul { p: self.rail }),
            Shape::Choice { .. } => None,
        };
        Ok(Decided {
            model: "sure-1".to_owned(),
            answers: questions.iter().map(answer).collect(),
            cost_usd: 0.0,
        })
    }
}

fn configure(cfg: &mut orch::Config, decider: Option<Arc<dyn Decider>>) {
    cfg.core.models = Some(provider::Catalog::default());
    cfg.core.chair_model = "fable".to_owned();
    cfg.core.judge_model = "fable".to_owned();
    cfg.core.decider = decider;
}

async fn chain_of(decider: Option<Arc<dyn Decider>>) -> Landed {
    chain_seeded(
        |_| {},
        |cfg| configure(cfg, decider),
        |wt| std::fs::write(wt.join("hello.txt"), "hello\n").unwrap(),
    )
    .await
}

fn sure(level: usize, rail: f64) -> Option<Arc<dyn Decider>> {
    Some(Arc::new(Sure {
        level,
        rail,
        sure: 0.97,
    }))
}

/// The reviewer the Chair hires to judge a worker's change.
async fn reviewer_of(t: &Landed) -> Card {
    let author = HireInput {
        agent: "research".to_owned(),
        brief: "Write the greeting.".to_owned(),
        why: "the owner wants a greeting".to_owned(),
        deliverable: "a file".to_owned(),
        ..HireInput::default()
    };
    let author = t.r.o.hire(&t.chair, author).await.unwrap();
    let review = HireInput {
        employee: author.employee.clone(),
        role: "reviewer".to_owned(),
        brief: "Review the greeting.".to_owned(),
        why: "a second pair of eyes".to_owned(),
        inputs: vec![HireInputRef {
            card_id: author.id.clone(),
            label: "the greeting".to_owned(),
        }],
        ..HireInput::default()
    };
    t.r.o.hire(&t.chair, review).await.unwrap()
}

async fn decisions_of(t: &Landed) -> Vec<String> {
    let home = t.chain.id.clone();
    let said = t.r.o.store.read(move |s| s.decisions(&home)).await.unwrap();
    said.into_iter()
        .filter(|d| d.kind == DecisionKind::Landing)
        .map(|d| d.text)
        .collect()
}

/// How many judgments of `question` were proved right and wrong, as the deciders are scored on them.
async fn proofs(t: &Landed, question: &judgment::Definition) -> (usize, usize) {
    let key = question.key();
    let scores = t.r.o.store.read(move |s| s.judgment_scores(&[key])).await.unwrap();
    let acting = scores.iter().filter(|s| s.decider == judgment::DEFAULT_DECIDER);
    (
        acting.clone().filter(|s| s.right).count(),
        acting.filter(|s| !s.right).count(),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn a_change_a_decider_is_sure_is_risky_and_alters_a_rail_gets_deeper_review_and_security() {
    let t = chain_of(sure(2, 0.97)).await;
    let judging = t.land().await;
    assert_eq!(judging.version.effort.as_deref(), Some("max"));
    assert_eq!(
        judging.version.gates,
        vec![Gate::Reviewer, Gate::Security],
        "no path of hello.txt is a rail: the decider added the gate"
    );
    assert_eq!(judging.version.judged.len(), 2);
    let said = decisions_of(&t).await;
    assert!(
        said.iter().any(|s| s.starts_with("Review and QC run at max effort")),
        "{said:?}"
    );
    assert!(
        said.iter().any(|s| s.starts_with("A security review is added")),
        "{said:?}"
    );
    let reviewer = reviewer_of(&t).await;
    assert_eq!(reviewer.effort, "max", "the judge thinks as hard as the change asks");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_decider_too_unsure_or_absent_leaves_the_review_as_the_kind_and_the_rails_make_it() {
    let unsure: Arc<dyn Decider> = Arc::new(Sure {
        level: 2,
        rail: 0.7,
        sure: 0.5,
    });
    for (decider, judged) in [(None, 0), (Some(unsure), 2)] {
        let t = chain_of(decider).await;
        let usual = t.chair.effort.clone();
        let judging = t.land().await;
        assert_eq!(judging.version.effort, None);
        assert_eq!(judging.version.gates, vec![Gate::Reviewer]);
        assert_eq!(
            judging.version.judged.len(),
            judged,
            "an answer too unsure to act is still kept, to be scored"
        );
        assert_eq!(reviewer_of(&t).await.effort, usual);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rail_the_path_rule_already_found_is_not_asked_again() {
    let t = chain_seeded(
        |_| {},
        |cfg| configure(cfg, sure(0, 0.97)),
        |wt| std::fs::write(wt.join("crates/auth/lib.rs"), "pub fn sign_in() { open() }\n").unwrap(),
    )
    .await;
    let judging = t.land().await;
    assert_eq!(judging.version.gates, vec![Gate::Reviewer, Gate::Security]);
    assert_eq!(judging.version.judged.len(), 1, "only how risky it is");
}

/// The security reviewer's verdict on the version judged now, as its tool records it.
async fn security_says(t: &Landed, verdict: &str) {
    let mut judge = Card::default();
    judge.id = "c_security".to_owned();
    judge.home.clone_from(&t.chain.id);
    judge.name = "Security".to_owned();
    judge.signs = Signs::Security;
    let judge = t.r.o.store.update(move |tx| tx.put_card(judge)).await.unwrap();
    let status = if verdict == "approve" { "met" } else { "not_met" };
    let input: VerdictInput = serde_json::from_value(serde_json::json!({
        "verdict": verdict,
        "notes": "read the change against the rails",
        "findings": [{"keyResult": "", "item": "the rail", "status": status, "evidence": "hello.txt:1", "code": "other"}],
    }))
    .unwrap();
    t.r.o.shipping.record_verdict(&judge, input).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_change_that_goes_live_proved_the_lowest_depth_enough_and_the_rails_question_nothing() {
    let t = chain_of(sure(0, 0.02)).await;
    t.landed_and_green().await;
    t.release_until(|r| r.state == ReleaseState::Live).await;
    assert_eq!(proofs(&t, &judgment::GATE_RISK).await, (1, 0));
    assert_eq!(proofs(&t, &judgment::GATE_TOUCHES_RAIL).await, (0, 0));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_change_the_owner_takes_back_proved_a_middle_depth_right_and_says_nothing_of_the_rails() {
    let t = chain_of(sure(1, 0.02)).await;
    let judging: Landing = t.land().await;
    t.approve(Gate::Reviewer).await;
    let landed = t
        .until(|l| l.id == judging.id, |l| l.state == LandingState::Landed)
        .await;
    t.r.o
        .shipping
        .revert_landing(
            by_owner("owner", "web"),
            RevertInput {
                landing: landed.id.clone(),
                why: "the greeting is wrong".to_owned(),
            },
        )
        .await
        .unwrap();
    t.until(
        |l| l.id == landed.id,
        |l| matches!(l.state, LandingState::Reverted { .. }),
    )
    .await;
    assert_eq!(
        proofs(&t, &judgment::GATE_RISK).await,
        (1, 0),
        "a risky change, and it got a harder look"
    );
    assert_eq!(proofs(&t, &judgment::GATE_TOUCHES_RAIL).await, (0, 0));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_security_gate_a_decider_added_scores_the_rails_question_by_what_it_finds() {
    for (verdict, rail, risk) in [("approve", (0, 1), (0, 0)), ("request_changes", (1, 0), (1, 0))] {
        let t = chain_of(sure(2, 0.97)).await;
        t.land().await;
        security_says(&t, verdict).await;
        assert_eq!(proofs(&t, &judgment::GATE_TOUCHES_RAIL).await, rail, "{verdict}");
        assert_eq!(
            proofs(&t, &judgment::GATE_RISK).await,
            risk,
            "a gate that sent the change back shows it was risky; an approval waits for the release"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_depth_that_would_not_raise_the_judges_changes_nothing_and_says_nothing() {
    let t = chain_of(sure(0, 0.02)).await;
    let judging = t.land().await;
    assert_eq!(judging.version.effort, None, "high is below Claude Code's default");
    assert_eq!(judging.version.judged.len(), 2, "but it is still scored");
    let said = decisions_of(&t).await;
    assert!(said.iter().all(|s| !s.starts_with("Review and QC run")), "{said:?}");
}
