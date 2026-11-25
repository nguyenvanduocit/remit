//! The decision layer's main path (ADR-0118), driven as a point that judges drives it: the acting decider's answer comes
//! back and is recorded with its shadows', a question it refused is left to its System Two, an outcome scores all of
//! them, and the Judgments page says so; a decider that took over by its numbers and then answers late is passed over
//! for the next; deciders that all fail leave the point to its System Two with nothing recorded.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use judgment::{Decided, Decider, DeciderError, Kind, Question};
use model::{JudgmentAnswer, JudgmentPick, JudgmentSubject, JudgmentUse, OutcomeSource, Proved, SubjectKind};
use serde_json::{Value, json};

use crate::tests::rig_of;

const LIQUID: &str = "liquid/d1.2-ultra";

/// Deciders that say yes: the default fairly surely, liquid all but surely (or past `judge::DECIDE_FOR` once `late`);
/// the free one fails as OpenRouter does with a 502. Each refuses whether memories contradict.
#[derive(Default)]
struct Sure {
    late: AtomicBool,
}

#[async_trait]
impl Decider for Sure {
    async fn decide(&self, decider: &str, _state: &Value, questions: &[Question]) -> Result<Decided, DeciderError> {
        let p = match decider {
            judgment::DEFAULT_DECIDER => 0.95,
            LIQUID if self.late.load(Ordering::SeqCst) => {
                tokio::time::sleep(Duration::from_secs(3)).await;
                0.99
            }
            LIQUID => 0.99,
            _ => return Err(DeciderError::Failed("OpenRouter answered 502".to_owned())),
        };
        let answer =
            |q: &Question| (q.key() != judgment::MEMORY_CONTRADICTS.key()).then_some(JudgmentAnswer::Noul { p });
        Ok(Decided {
            model: format!("{decider}-20261116"),
            answers: questions.iter().map(answer).collect(),
            cost_usd: 0.002,
        })
    }
}

/// Deciders that all fail.
struct Down;

#[async_trait]
impl Decider for Down {
    async fn decide(&self, _: &str, _: &Value, _: &[Question]) -> Result<Decided, DeciderError> {
        Err(DeciderError::Failed("OpenRouter answered 502".to_owned()))
    }
}

fn same_claim() -> Question {
    let kind = Kind::noul(
        "One could be merged into the other without losing advice the other gives.",
        "Each gives advice the other does not.",
    )
    .unwrap();
    Question::new(&judgment::MEMORY_SAME_CLAIM, kind).unwrap()
}

fn contradicts() -> Question {
    let kind = Kind::noul("Following one breaks the other.", "Both can be followed.").unwrap();
    Question::new(&judgment::MEMORY_CONTRADICTS, kind).unwrap()
}

fn pair() -> JudgmentSubject {
    JudgmentSubject {
        kind: SubjectKind::MemoryPair,
        id: "mem_a+mem_b".to_owned(),
        home: "ch_librarian".to_owned(),
    }
}

/// The Judgments page once its first question counts answers judgments in all.
async fn page_with(o: &orch::Orchestrator, judgments: i64) -> wire::app::Judgments {
    for _ in 0..300 {
        let view = o.judgments().await.unwrap();
        let all: i64 = view
            .questions
            .first()
            .map_or(0, |q| q.details.deciders.iter().map(|d| d.judgments).sum());
        if all >= judgments {
            return view;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the page never counted {judgments} answers")
}

#[tokio::test(flavor = "multi_thread")]
async fn an_answer_acts_is_recorded_with_its_shadows_and_its_outcome_scores_them() {
    let r = rig_of(|cfg| cfg.core.decider = Some(Arc::new(Sure::default())));
    let state = json!({"a": "record_verdict needs a clean worktree", "b": "commit first", "apiKey": "sk-or-1234"});
    let judged = r.o.judge(pair(), &state, vec![same_claim(), contradicts()]).await;
    let [Some(j), None] = judged.as_slice() else {
        panic!("the refused question has no answer, the other has: {judged:?}")
    };
    assert_eq!(
        (&j.pick, j.p, j.used),
        (&JudgmentPick::Yes, 0.95, JudgmentUse::Acted),
        "0.95 is over the provisional 0.9"
    );

    // The shadows answer in the background: liquid is recorded, the free one failed and is not.
    page_with(&r.o, 2).await;
    r.o.record_outcome(&j.judgment, Proved::Right, OutcomeSource::Librarian)
        .await
        .unwrap();
    let view = r.o.judgments().await.unwrap();
    let [q] = view.questions.as_slice() else {
        panic!("only the answered question is on the page: {view:?}")
    };
    assert_eq!(
        (q.key.as_str(), q.title.as_str()),
        ("memory.same_claim@1", "Finding duplicate memories")
    );
    assert_eq!(
        q.summary,
        "Not settled yet: until 30 results are known it decides alone only when at least 90% sure."
    );
    assert_eq!(
        q.answered_by,
        "Answered by Jev 1.14. Liquid D1.2 Ultra answers beside it, to show whether it would do better."
    );
    let standings: Vec<(&str, &str, i64, Option<f64>)> = q
        .details
        .deciders
        .iter()
        .map(|d| (d.decider.as_str(), d.role.as_str(), d.outcomes, d.brier))
        .collect();
    let brier = |p: f64| Some((1.0 - p) * (1.0 - p));
    assert_eq!(
        standings,
        vec![
            (judgment::DEFAULT_DECIDER, "Answers it", 1, brier(0.95)),
            (LIQUID, "Measured beside it", 1, brier(0.99)),
        ],
        "the shadow is scored on the acting pick"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_decider_that_took_over_and_answers_late_is_passed_over_for_the_next() {
    let sure = Arc::new(Sure::default());
    let decider = Arc::clone(&sure);
    let r = rig_of(move |cfg| cfg.core.decider = Some(decider));
    // Thirty answers proved right: liquid, surer, earns the better standing and takes the question over.
    for i in 0..judgment::MIN_OUTCOMES {
        let judged = r.o.judge(pair(), &json!({"round": i}), vec![same_claim()]).await;
        let [Some(j)] = judged.as_slice() else {
            panic!("the default answers")
        };
        r.o.record_outcome(&j.judgment, Proved::Right, OutcomeSource::Librarian)
            .await
            .unwrap();
    }
    let view = page_with(&r.o, 60).await;
    assert!(
        view.questions[0].answered_by.starts_with(
            "Answered by Liquid D1.2 Ultra \u{2014} switched from Jev 1.14 because its answers proved more reliable."
        ),
        "{}",
        view.questions[0].answered_by
    );

    sure.late.store(true, Ordering::SeqCst);
    let judged = r.o.judge(pair(), &json!({"round": "late"}), vec![same_claim()]).await;
    let [Some(j)] = judged.as_slice() else {
        panic!("the next decider answers for the late one: {judged:?}")
    };
    assert_eq!(
        (j.p, j.used),
        (0.95, JudgmentUse::Acted),
        "held to the default's own threshold, 0.95"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn deciders_that_all_fail_leave_the_point_to_its_system_two_and_record_nothing() {
    let r = rig_of(|cfg| cfg.core.decider = Some(Arc::new(Down)));
    let got = r.o.judge(pair(), &json!("A and B"), vec![same_claim()]).await;
    assert_eq!(got, vec![None]);
    assert!(r.o.judgments().await.unwrap().questions.is_empty());
}
fn owner() -> model::Actor {
    model::by_owner("owner", "test")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_pinned_decider_only_informs_until_it_has_its_own_results_and_the_page_says_so() {
    let r = rig_of(|cfg| cfg.core.decider = Some(Arc::new(Sure::default())));
    let said =
        r.o.set_decider(
            &owner(),
            "memory.same_claim@1",
            LIQUID,
            "it found every duplicate we labelled",
        )
        .await
        .unwrap();
    assert_eq!(
        said,
        "Liquid D1.2 Ultra has 0 of the 30 results it needs on Finding duplicate memories, so for now it only informs: it is asked beside Jev 1.14, which still decides, and takes over once it has them."
    );
    let judged = r.o.judge(pair(), &json!({"a": "x"}), vec![same_claim()]).await;
    let [Some(j)] = judged.as_slice() else {
        panic!("{judged:?}")
    };
    assert_eq!(
        j.decider,
        judgment::DEFAULT_DECIDER,
        "a model nobody has measured on the question never decides it alone"
    );
    let view = page_with(&r.o, 2).await;
    let q = &view.questions[0];
    assert!(
        q.answered_by.contains(
            "You chose Liquid D1.2 Ultra (you: \u{201c}it found every duplicate we labelled\u{201d}); it has 0 of the 30 results it needs, so for now it only informs"
        ),
        "{}",
        q.answered_by
    );
    // Thirty answers proved right measure it, beside the one that decides.
    r.o.record_outcome(&j.judgment, Proved::Right, OutcomeSource::Librarian)
        .await
        .unwrap();
    for i in 1..judgment::MIN_OUTCOMES {
        let judged = r.o.judge(pair(), &json!({"round": i}), vec![same_claim()]).await;
        let [Some(j)] = judged.as_slice() else {
            panic!("{judged:?}")
        };
        r.o.record_outcome(&j.judgment, Proved::Right, OutcomeSource::Librarian)
            .await
            .unwrap();
        page_with(&r.o, i64::try_from(2 * (i + 1)).unwrap()).await;
    }
    let judged = r.o.judge(pair(), &json!({"a": "measured"}), vec![same_claim()]).await;
    let [Some(j)] = judged.as_slice() else {
        panic!("{judged:?}")
    };
    assert_eq!(j.decider, LIQUID, "measured, the pinned model answers first");
    let view = page_with(&r.o, 62).await;
    let q = &view.questions[0];
    assert!(
        q.answered_by.starts_with(
            "Answered by Liquid D1.2 Ultra, chosen by you: \u{201c}it found every duplicate we labelled\u{201d}."
        ),
        "{}",
        q.answered_by
    );
    assert!(
        q.details
            .facts
            .iter()
            .any(|f| f.label == "Model chosen by" && f.value.contains("you, ") && f.value.contains("every duplicate")),
        "the fold keeps who and when: {:?}",
        q.details.facts
    );
    r.o.set_decider(&owner(), "memory.same_claim", "default", "back to the measured one")
        .await
        .unwrap();
    let judged = r.o.judge(pair(), &json!({"a": "y"}), vec![same_claim()]).await;
    let [Some(j)] = judged.as_slice() else {
        panic!("{judged:?}")
    };
    assert_eq!(
        j.decider, LIQUID,
        "unpinned, the standings rank it first: it has the better results"
    );
}

fn tier_can() -> Question {
    let kind = Kind::noul("A cheaper model can do the work.", "The work needs a stronger model.").unwrap();
    Question::new(&judgment::HIRE_TIER_CAN, kind).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lower_bar_lets_a_decider_decide_alone_at_a_certainty_the_default_bar_holds_back() {
    let r = rig_of(|cfg| cfg.core.decider = Some(Arc::new(Sure::default())));
    // Thirty answers at 95%, of which 22 proved right: 73% precision, short of the 80% this question is held to.
    for i in 0..judgment::MIN_OUTCOMES {
        let judged = r.o.judge(pair(), &json!({"round": i}), vec![tier_can()]).await;
        let [Some(j)] = judged.as_slice() else {
            panic!("{judged:?}")
        };
        let proved = if i < 22 { Proved::Right } else { Proved::Wrong };
        r.o.record_outcome(&j.judgment, proved, OutcomeSource::Librarian)
            .await
            .unwrap();
    }
    let judged = r.o.judge(pair(), &json!({"round": "held"}), vec![tier_can()]).await;
    let [Some(held)] = judged.as_slice() else {
        panic!("{judged:?}")
    };
    assert_eq!(
        held.used,
        JudgmentUse::Escalated,
        "no certainty has proved right 80% of the time"
    );
    assert_eq!(held.threshold, None, "and the judgment says there was no bar to reach");

    let said =
        r.o.set_target_precision(
            &owner(),
            "hire.tier_can",
            Some(70),
            "a hire sent back climbs a tier, so a wrong cheap choice costs little",
        )
        .await
        .unwrap();
    assert!(
        said.starts_with("Choosing the model for a hire now decides alone only at a level of certainty that has proved right 70% of the time.")
            && !said.contains("provisional"),
        "{said}"
    );
    let judged = r.o.judge(pair(), &json!({"round": "lower"}), vec![tier_can()]).await;
    let [Some(acted)] = judged.as_slice() else {
        panic!("{judged:?}")
    };
    assert_eq!(acted.used, JudgmentUse::Acted, "73% reaches the 70% the owner set");
    assert_eq!(
        acted.threshold,
        Some(0.95),
        "the bar it reached is kept with its answer"
    );
    let view = r.o.judgments().await.unwrap();
    assert!(
        view.questions[0].summary.ends_with(
            "The bar of 70% was set by you: \u{201c}a hire sent back climbs a tier, so a wrong cheap choice costs little\u{201d}."
        ),
        "{}",
        view.questions[0].summary
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn only_the_owner_steers_and_a_setting_that_cannot_be_kept_is_refused() {
    let r = rig_of(|cfg| cfg.core.decider = Some(Arc::new(Sure::default())));
    let agent = model::by_agent("c_1");
    let q = "memory.same_claim";
    assert!(r.o.set_decider(&agent, q, LIQUID, "because").await.is_err());
    assert!(
        r.o.set_decider(&owner(), q, "jev", "because").await.is_err(),
        "not a model id"
    );
    assert!(r.o.set_decider(&owner(), q, LIQUID, " ").await.is_err(), "no why");
    assert!(r.o.set_decider(&owner(), "no.such", LIQUID, "because").await.is_err());
    assert!(r.o.set_target_precision(&owner(), q, Some(100), "sure").await.is_err());
    assert!(
        r.o.set_target_precision(&owner(), "agent.ask", Some(90), "sure")
            .await
            .is_err(),
        "an agent's own question has no results to hold a bar to"
    );
    assert!(
        r.o.set_decider(&owner(), "agent.ask", LIQUID, "because").await.is_err(),
        "an agent's own question has no results, so a pin would never be used"
    );
    let below =
        r.o.set_target_precision(&owner(), q, Some(80), "sure")
            .await
            .unwrap_err();
    assert!(
        below.to_string().contains("at least 90%") && below.to_string().contains("a lesson merged away"),
        "a wrong merge loses a lesson, so the floor says why: {below}"
    );
    let informs =
        r.o.set_target_precision(&owner(), "inbox.option", Some(80), "sure")
            .await
            .unwrap_err();
    assert!(informs.to_string().contains("only informs"), "{informs}");
    let said =
        r.o.set_target_precision(&owner(), "card.progress", Some(95), "a note costs the Chair a read")
            .await
            .unwrap();
    assert!(
        said.contains("speaks to the card's Chair only at a level of certainty that has proved right 95%")
            && said.contains("0 of the 30 results it needs")
            && said.contains("provisional 90% still applies"),
        "{said}"
    );
}

/// A decider that keeps the instructions and the state it was last asked.
#[derive(Default)]
struct Spy {
    seen: std::sync::Mutex<Vec<(String, Value)>>,
}

#[async_trait]
impl Decider for Spy {
    async fn decide(&self, _: &str, state: &Value, questions: &[Question]) -> Result<Decided, DeciderError> {
        let mut seen = self.seen.lock().unwrap();
        seen.extend(questions.iter().map(|q| (format!("{q:?}"), state.clone())));
        Ok(Decided {
            model: "spy-1".to_owned(),
            answers: questions
                .iter()
                .map(|_| Some(JudgmentAnswer::Noul { p: 0.9 }))
                .collect(),
            cost_usd: 0.0,
        })
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_agent_asks_a_decider_in_its_own_words_and_the_answer_is_recorded_on_its_card_with_no_shadow() {
    let spy = Arc::new(Spy::default());
    let decider = Arc::clone(&spy);
    let r = rig_of(move |cfg| cfg.core.decider = Some(decider));
    let chain = r.create("rq_ask", "Find the failing deploy").await;
    let chair = chain.chair_id.clone();
    let id = chain.id.clone();
    r.o.store
        .update(move |tx| {
            let mut c = tx.chain(&id)?;
            c.capabilities.push(chains::cap_names::CAP_JUDGMENT.to_owned());
            tx.put_chain(c).map(|_| ())
        })
        .await
        .unwrap();
    let tools = r.o.tools_for(&chair).await.unwrap();
    const LEAK: &str = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
    let args = json!({
        "state": {"log": "deploy 41 failed: exit 1", "apiKey": "sk-or-1234", (LEAK): "x"},
        "questions": [{
            "ask": format!("Does this log show the deploy failing? (token {LEAK})"),
            "form": "yes_no",
            "yes": format!("The log reports a failed deploy, as {LEAK} saw."),
            "no": "The log reports no failure."
        }]
    });
    let out = tools
        .call("ask_decider", args.as_object().cloned())
        .await
        .unwrap()
        .unwrap();
    let answers: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(answers[0]["answer"], "yes");
    assert_eq!(answers[0]["decider"], "Jev 1.14");

    let (asked, state) = spy.seen.lock().unwrap().first().cloned().unwrap();
    assert!(asked.contains("Does this log show the deploy failing? "), "{asked}");
    assert!(
        !asked.contains(LEAK) && !state.to_string().contains(LEAK),
        "a token leaves in neither the question nor a key: {asked} {state}"
    );
    assert!(
        !state.to_string().contains("sk-or-1234"),
        "a credential never leaves: {state}"
    );

    // A question that cannot be answered is refused in words the agent can fix.
    let bad = json!({"state": "x", "questions": [{"ask": "Is it?", "form": "yes_no"}]});
    let refused = tools
        .call("ask_decider", bad.as_object().cloned())
        .await
        .unwrap()
        .unwrap_err();
    assert!(refused.to_string().contains("question 1"), "{refused}");

    tokio::time::sleep(Duration::from_millis(300)).await;
    let tallies = r.o.store.read(|s| s.judgment_tallies()).await.unwrap();
    let asked: Vec<_> = tallies
        .iter()
        .filter(|t| t.question == judgment::AGENT_ASK.key())
        .map(|t| (t.decider.as_str(), t.used, t.judgments))
        .collect();
    assert_eq!(
        asked,
        vec![(judgment::DEFAULT_DECIDER, JudgmentUse::Informed, 1)],
        "one answer, on the card, and no shadow: nothing would score it"
    );
}
