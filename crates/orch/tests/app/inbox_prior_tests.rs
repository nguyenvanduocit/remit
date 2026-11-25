//! An item for the owner (ADR-0118, point 5), driven as the Assistant drives it: the deciders' answers are asked before
//! its turn and come back as a prior in its words, a decider that is sure of an option the Assistant did not take sends
//! its answer to a second opinion, the owner's overturn scores the pick of the option the Assistant took, and a second
//! opinion's own answer scores the picks it has a reference for. Nothing here changes what the Assistant decides.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use judgment::{Decided, Decider, DeciderError, Question, Shape};
use model::{
    Decision, DecisionKind, InboxItem, InboxKind, InboxOption, InboxStatus, Judgment, JudgmentAnswer, OutcomeSource,
    Proved, SecondOpinion, by_assistant,
};
use serde_json::{Value, json};

use crate::tests::{Rig, owner, rig_with as rig_with_launcher};

/// A decider that is sure the owner would take option "allow" of any item and that answering cannot be undone.
#[derive(Default)]
struct SureOfAllow {
    calls: AtomicUsize,
}

#[async_trait]
impl Decider for SureOfAllow {
    async fn decide(&self, _: &str, state: &Value, questions: &[Question]) -> Result<Decided, DeciderError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert!(state.get("item").is_some(), "the item is what is asked about");
        let answer = |q: &Question| match q.kind().shape() {
            Shape::Choice { options } => Some(JudgmentAnswer::Choice {
                choice: "allow".to_owned(),
                probabilities: options
                    .iter()
                    .map(|o| (o.name.clone(), if o.name == "allow" { 0.95 } else { 0.05 }))
                    .collect(),
                confidence: 0.95,
            }),
            Shape::Noul { .. } => Some(JudgmentAnswer::Noul { p: 0.04 }),
            Shape::Score { .. } => None,
        };
        Ok(Decided {
            model: "sure-1".to_owned(),
            answers: questions.iter().map(answer).collect(),
            cost_usd: 0.0,
        })
    }
}

/// A launcher whose sessions never open, so a judge's card stays where it was started and its second opinion stays open
/// for the test to answer (`Nothing` fails the start, which ends the opinion at once).
struct Hangs;

impl agent::Launcher for Hangs {
    fn start(
        &self,
        _spec: agent::Spec,
        _h: Arc<dyn agent::Handler>,
    ) -> agent::BoxFuture<'_, Result<Box<dyn agent::Session>, agent::Error>> {
        Box::pin(std::future::pending())
    }
}

fn rig_with(decider: Option<Arc<dyn Decider>>) -> Rig {
    rig_with_launcher(
        |cfg| cfg.core.decider = decider,
        Arc::new(Hangs),
        Arc::new(|| crate::tests::NOW),
    )
}

fn option(id: &str, name: &str) -> InboxOption {
    InboxOption {
        id: id.to_owned(),
        name: name.to_owned(),
        ..InboxOption::default()
    }
}

/// An open question of the Chair of a new chain, with the options allow and refuse, raised as an agent raises it: stored
/// with the fact `owner.asked`, which the Tick delivers to the Assistant (`fire_assist`).
async fn item_of(r: &Rig, kind: InboxKind) -> InboxItem {
    r.o.core.ensure_assistant().await.unwrap();
    // as the server does at start: the firing is answered by sub_assist_owner
    r.o.pipeline.seed_subscriptions().await.unwrap();
    let chain = r.create("rq_chain", "Ship the report").await;
    let it = InboxItem {
        id: "in_1".to_owned(),
        home: chain.id.clone(),
        card_id: chain.chair_id.clone(),
        kind,
        title: "Send the report to the customer?".to_owned(),
        detail: "The report is ready.".to_owned(),
        options: vec![option("allow", "Send it"), option("refuse", "Hold it back")],
        status: InboxStatus::Open,
        ..InboxItem::default()
    };
    let door = r.o.core.door();
    let put = it.clone();
    let stored =
        r.o.store
            .update(move |tx| chains::inbox::put_asked(tx, &door, put))
            .await
            .unwrap();
    r.o.tick().await;
    // the delivery runs beside the Tick: wait for the message it stores for the Assistant (a test fails after 10 s)
    for _ in 0..200 {
        if told_to_assistant(r).await.contains("[for you to decide") {
            return stored;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("the Assistant was not put the item");
}

/// What the Assistant was told, all of its messages.
async fn told_to_assistant(r: &Rig) -> String {
    let id = r.o.core.assistant_card().await.unwrap().id;
    let events = r.o.store.read(move |s| s.card_events(&id, 100)).await.unwrap();
    format!("{events:?}")
}

/// The judgments the deciders gave item, by question.
async fn judged(r: &Rig, question: &judgment::Definition) -> Judgment {
    let keys = [question.key()];
    let all =
        r.o.store
            .read(move |s| s.item_judgments_of("in_1", &keys))
            .await
            .unwrap();
    all.into_iter().next().unwrap()
}

/// The Assistant answers the item with option took, as `answer_inbox` does, and its answer's decision.
async fn answered(r: &Rig, it: &InboxItem, took: &str) -> Decision {
    let by = assistant(r).await;
    r.o.answer_item(&it.id, took, "", by.clone()).await.unwrap();
    r.o.record_assist(&it.id, Some(took), "he would", by).await.unwrap();
    let home = it.home.clone();
    let said = r.o.store.read(move |s| s.decisions(&home)).await.unwrap();
    said.into_iter().find(|d| d.kind == DecisionKind::Assist).unwrap()
}

/// The one second opinion started, and its outcome source as a judgment that proves a pick gets: sampled or targeted.
async fn opinion_started(r: &Rig, answer: &Decision) -> SecondOpinion {
    let opinions = r.o.store.read(|s| s.recent_second_opinions(10)).await.unwrap();
    assert_eq!(
        opinions.len(),
        1,
        "the decider was 95% sure of allow; the Assistant refused"
    );
    let opinion = opinions.into_iter().next().unwrap();
    assert_eq!(
        opinion.targeted,
        !chains::judge::sampled(&answer.id, 8),
        "targeted unless the random share took it"
    );
    opinion
}

/// The second judge gives its answer through its tool, as its session does.
async fn judge_says(r: &Rig, opinion: &SecondOpinion, option: &str, undoable: bool) {
    let tools = r.o.tools_for(&opinion.judge).await.unwrap();
    let args = json!({"option_id": option, "why": "the customer has not paid", "undoable": undoable});
    let args = args.as_object().cloned().unwrap();
    tools.call("second_opinion", Some(args)).await.unwrap().unwrap();
}

fn source_of(opinion: &SecondOpinion) -> OutcomeSource {
    if opinion.targeted {
        OutcomeSource::SecondOpinionTargeted
    } else {
        OutcomeSource::SecondOpinion
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_assistant_is_told_what_the_deciders_say_of_an_item_as_a_prior_it_decides_on_its_own() {
    let decider = Arc::new(SureOfAllow::default());
    let r = rig_with(Some(decider.clone()));
    item_of(&r, InboxKind::Question).await;
    let told = told_to_assistant(&r).await;
    assert!(
        told.contains("[for you to decide")
            && told.contains("Jev 1.14 would take allow (Send it), 95% sure")
            && told.contains("answering cannot be undone later at little cost, 96% sure")
            && told.contains("a prior, not a decision"),
        "{told}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn no_decider_leaves_the_assistant_with_the_item_alone_and_a_secret_is_never_put_to_one() {
    let r = rig_with(None);
    item_of(&r, InboxKind::Question).await;
    let told = told_to_assistant(&r).await;
    assert!(
        told.contains("[for you to decide") && !told.contains("Fast deciders"),
        "{told}"
    );
    let decider = Arc::new(SureOfAllow::default());
    let r = rig_with(Some(decider.clone()));
    let secret = InboxItem {
        kind: InboxKind::Credential,
        ..item_of(&r, InboxKind::Question).await
    };
    let before = decider.calls.load(Ordering::SeqCst);
    assert_eq!(
        r.o.core.prior_for_item(&secret, "the Chair", "").await.words(&secret),
        ""
    );
    assert_eq!(decider.calls.load(Ordering::SeqCst), before, "no decider was asked");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_answer_that_parts_from_a_sure_decider_is_re_decided_by_a_second_judge_and_marked_targeted() {
    let r = rig_with(Some(Arc::new(SureOfAllow::default())));
    let it = item_of(&r, InboxKind::Question).await;
    let answer = answered(&r, &it, "refuse").await;
    assert_eq!(opinion_started(&r, &answer).await.item, it.id);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_second_judges_answer_scores_the_picks_it_has_a_reference_for_and_a_targeted_one_is_no_standing() {
    let r = rig_with(Some(Arc::new(SureOfAllow::default())));
    let it = item_of(&r, InboxKind::Question).await;
    let answer = answered(&r, &it, "refuse").await;
    let opinion = opinion_started(&r, &answer).await;
    judge_says(&r, &opinion, "refuse", false).await;
    let option = judged(&r, &judgment::INBOX_OPTION).await.outcome.unwrap();
    assert_eq!(
        (option.proved, option.source),
        (Proved::Wrong, source_of(&opinion)),
        "the decider picked allow, the second judge refuse"
    );
    let reversible = judged(&r, &judgment::INBOX_REVERSIBLE).await.outcome.unwrap();
    assert_eq!(reversible.proved, Proved::Right, "both said it cannot be undone");
    let scores = r.o.store.read(|s| s.judgment_scores(&[])).await.unwrap();
    // The shadows asked beside the acting decider are stored in the background and scored on its pick as they are
    // stored, so how many scores a sampled opinion gives is not fixed; that it gives some, and a targeted one none, is.
    assert_eq!(
        scores.is_empty(),
        opinion.targeted,
        "only a sampled opinion is a standing"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_owner_overturning_the_option_a_decider_picked_proves_it_wrong_and_another_option_proves_nothing() {
    for (took, proved) in [("allow", Some(Proved::Wrong)), ("refuse", None)] {
        let r = rig_with(Some(Arc::new(SureOfAllow::default())));
        let it = item_of(&r, InboxKind::Question).await;
        let answer = answered(&r, &it, took).await;
        assert_eq!(answer.option, took);
        r.o.overturn(owner("rq_overturn"), answer.id.clone(), "no, hold it".to_owned())
            .await
            .unwrap();
        let outcome = judged(&r, &judgment::INBOX_OPTION).await.outcome;
        assert_eq!(outcome.as_ref().map(|o| o.proved), proved);
        assert_eq!(outcome.map(|o| o.source), proved.map(|_| OutcomeSource::Owner));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_owners_word_is_final_whichever_comes_first_the_overturn_or_the_judges_answer() {
    for judge_first in [true, false] {
        let r = rig_with(Some(Arc::new(SureOfAllow::default())));
        let it = item_of(&r, InboxKind::Question).await;
        let answer = answered(&r, &it, "refuse").await;
        let opinion = opinion_started(&r, &answer).await;
        if judge_first {
            judge_says(&r, &opinion, "refuse", false).await;
        }
        r.o.overturn(owner("rq_overturn"), answer.id.clone(), "no, send it".to_owned())
            .await
            .unwrap();
        if !judge_first {
            judge_says(&r, &opinion, "refuse", false).await;
        }
        let option = judged(&r, &judgment::INBOX_OPTION).await;
        assert_eq!(
            option.outcome, None,
            "the judge's reference (refuse) was overturned, so it proves nothing of allow (judge first: {judge_first})"
        );
        let reversible = judged(&r, &judgment::INBOX_REVERSIBLE).await.outcome.unwrap();
        assert_eq!(reversible.proved, Proved::Right, "the overturn says nothing of it");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_opinion_never_replaces_what_the_owner_proved() {
    let r = rig_with(Some(Arc::new(SureOfAllow::default())));
    let it = item_of(&r, InboxKind::Question).await;
    let (id, at) = (judged(&r, &judgment::INBOX_OPTION).await.id, r.o.now());
    let owners = model::JudgmentOutcome {
        proved: Proved::Wrong,
        source: OutcomeSource::Owner,
        at,
    };
    let opinion = SecondOpinion {
        item: it.id.clone(),
        option: "allow".to_owned(),
        ..SecondOpinion::default()
    };
    r.o.store
        .update(move |tx| {
            tx.set_judgment_outcome(&id, &owners)?;
            chains::inbox_prior::opinion_given(tx, &opinion, at)
        })
        .await
        .unwrap();
    let option = judged(&r, &judgment::INBOX_OPTION).await.outcome.unwrap();
    assert_eq!((option.proved, option.source), (Proved::Wrong, OutcomeSource::Owner));
}

/// The Assistant, made, as the actor of its answers.
async fn assistant(r: &Rig) -> model::Actor {
    r.o.core.ensure_assistant().await.unwrap();
    by_assistant(&r.o.core.assistant_card().await.unwrap().id)
}
