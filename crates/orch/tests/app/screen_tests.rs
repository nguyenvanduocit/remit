//! A subscription's screen (ADR-0118, point 2), driven as the Tick drives it: a start on an event whose screen a decider
//! answers drops the readings it is sure need no chain (they are near-misses, counted) and starts a chain for the rest,
//! the twentieth of the dropped excepted; a screen only ever drops, so a missing decider changes nothing. What the chain
//! a screened reading started does, when it finishes, scores the decider's pick.

use std::sync::Arc;

use async_trait::async_trait;
use chains::apply::Records;
use chains::ingest::Write;
use chains::screen::{AUDIT_PERCENT, SCREENED_PER_PASS};
use chains::step::FinishCmd;
use chains::subscribe::subscribe;
use judgment::{Decided, Decider, DeciderError, Question};
use model::{
    ActionKind, ChainOrigin, ForeignJson, Judgment, JudgmentAnswer, JudgmentPick, JudgmentUse, OutcomeSource,
    OwnerKind, Proved, SignalKey, SignalPattern, SkipReason, Start, StartFrom, StartSpec, Subscription,
    SubscriptionAction, SubscriptionOwner, Unjudged, Writer, WriterKind,
};
use serde_json::{Value, json};
use store::FiringFilter;

use crate::tests::{Rig, rig_of};

const SUB: &str = "sub_screen";
const KEY: &str = "event:alerts";
const SCREEN: &str = "The alert is one a person must fix, not a retry that already succeeded.";

/// A decider sure that an event that says "retry" needs no chain and that every other does.
struct Retries;

#[async_trait]
impl Decider for Retries {
    async fn decide(&self, decider: &str, state: &Value, questions: &[Question]) -> Result<Decided, DeciderError> {
        let p = if state.to_string().contains("retry") {
            0.03
        } else {
            0.97
        };
        Ok(Decided {
            model: format!("{decider}-20261120"),
            answers: questions.iter().map(|_| Some(JudgmentAnswer::Noul { p })).collect(),
            cost_usd: 0.0,
        })
    }
}

fn rig_with(decider: Option<Arc<dyn Decider>>) -> Rig {
    rig_of(|cfg| cfg.core.decider = decider)
}

/// A start on `KEY` that screens, made at the head of an empty log, and `bodies` written as events after it.
async fn emit_into_a_screened_start(r: &Rig, bodies: &[&str]) {
    let sub = Subscription {
        id: SUB.to_owned(),
        owner: SubscriptionOwner {
            kind: OwnerKind::Remit,
            id: String::new(),
        },
        r#match: SignalPattern(KEY.to_owned()),
        note: "Start a chain for each alert".to_owned(),
        screen: SCREEN.to_owned(),
        action: SubscriptionAction {
            kind: ActionKind::Start,
            start: Some(StartSpec {
                title: "Alert".to_owned(),
                goal: "Find out why it alerted and fix it.".to_owned(),
                origin: ChainOrigin::Event,
                ..StartSpec::default()
            }),
            ..SubscriptionAction::default()
        },
        ..Subscription::default()
    };
    let now = r.o.now();
    let from = Start {
        from: StartFrom::Seq,
        seq: 1,
        ..Start::default()
    };
    r.o.store
        .update(move |tx| subscribe(tx, sub, &from, now))
        .await
        .unwrap();
    let door = r.o.core.door();
    let writes: Vec<Write> = bodies
        .iter()
        .enumerate()
        .map(|(i, body)| Write {
            key: SignalKey(KEY.to_owned()),
            value: Some(ForeignJson(json!({"event": {"body": body}}))),
            dedupes: vec![format!("alerts:{i}")],
            writer: Writer {
                kind: WriterKind::Cli,
                id: "test".to_owned(),
            },
            ..Write::default()
        })
        .collect();
    r.o.store
        .update(move |tx| {
            for w in writes {
                door.ingest(tx, w)?;
            }
            Ok::<_, chains::Error>(())
        })
        .await
        .unwrap();
}

/// The audit's sample among the places in the log `seqs`, as the screen draws it.
fn audited(seqs: &[i64]) -> Vec<i64> {
    seqs.iter()
        .copied()
        .filter(|seq| chains::judge::sampled(&chains::screen::subject_of(SUB, *seq, SCREEN).id, AUDIT_PERCENT))
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_start_whose_screen_a_decider_is_sure_of_drops_what_needs_no_chain_and_audits_a_few() {
    let r = rig_with(Some(Arc::new(Retries)));
    let bodies = [
        "pipeline failed on main",
        "retry 2 succeeded",
        "retry 3 succeeded",
        "retry 4 succeeded",
        "retry 5 succeeded",
        "deploy failed in production",
        "retry 7 succeeded",
        "retry 8 succeeded",
    ];
    emit_into_a_screened_start(&r, &bodies).await;
    r.o.tick().await;

    let (misses, firings, sub) =
        r.o.store
            .read(|s| {
                Ok::<_, store::Error>((
                    s.near_misses(SUB, 50)?,
                    s.firings(
                        &FiringFilter {
                            subscription: SUB.to_owned(),
                            ..FiringFilter::default()
                        },
                        50,
                    )?,
                    s.subscription(SUB)?,
                ))
            })
            .await
            .unwrap();
    assert!(misses.iter().all(|m| m.reason == SkipReason::Screened), "{misses:?}");
    assert!(
        misses.iter().all(|m| m.detail == "97% sure it needs no chain"),
        "{misses:?}"
    );
    let dropped: Vec<i64> = misses.iter().map(|m| m.seq).collect();
    let fired: Vec<i64> = firings.iter().flat_map(|f| f.seqs.clone()).collect();
    assert_eq!(dropped.len() + fired.len(), bodies.len(), "every event was judged once");
    assert!(!dropped.is_empty(), "the decider dropped events that needed no chain");
    // the dropped are the events the decider said no to, less the audit's twentieth, which fire with the two it said yes to
    assert!(
        audited(&dropped).is_empty(),
        "an audited event was dropped: {dropped:?}"
    );
    let all: Vec<i64> = dropped.iter().chain(&fired).copied().collect();
    assert!(audited(&all).iter().all(|seq| fired.contains(seq)));
    assert!(fired.len() >= 2, "the two events that needed a chain fired: {fired:?}");
    assert_eq!(sub.tally.screened, i64::try_from(misses.len()).unwrap());
    assert!(
        firings.iter().all(|f| !f.screen.is_empty()),
        "every firing keeps the judgment of its reading: {firings:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn with_no_decider_a_screen_changes_nothing() {
    let r = rig_with(None);
    emit_into_a_screened_start(&r, &["retry 1 succeeded", "retry 2 succeeded", "pipeline failed"]).await;
    r.o.tick().await;

    let (misses, firings) =
        r.o.store
            .read(|s| {
                Ok::<_, store::Error>((
                    s.near_misses(SUB, 50)?,
                    s.firings(
                        &FiringFilter {
                            subscription: SUB.to_owned(),
                            ..FiringFilter::default()
                        },
                        50,
                    )?,
                ))
            })
            .await
            .unwrap();
    assert!(misses.is_empty(), "{misses:?}");
    assert_eq!(firings.len(), 3);
    assert!(firings.iter().all(|f| f.screen.is_empty()));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_burst_is_screened_a_pass_at_a_time_and_never_fires_unscreened() {
    let r = rig_with(Some(Arc::new(Retries)));
    let n = SCREENED_PER_PASS + 5;
    let bodies: Vec<String> = (0..n).map(|i| format!("pipeline {i} failed")).collect();
    let refs: Vec<&str> = bodies.iter().map(String::as_str).collect();
    emit_into_a_screened_start(&r, &refs).await;
    r.o.tick().await;

    let firings =
        r.o.store
            .read(|s| {
                s.firings(
                    &FiringFilter {
                        subscription: SUB.to_owned(),
                        ..FiringFilter::default()
                    },
                    100,
                )
            })
            .await
            .unwrap();
    assert_eq!(firings.len(), SCREENED_PER_PASS, "the rest wait for the next pass");
    assert!(firings.iter().all(|f| !f.screen.is_empty()));

    // a pass whose subscription changed while the deciders answered (a delivery counted its first firing) is evaluated
    // again by the next
    let mut after = firings;
    for _ in 0..5 {
        if after.len() == n {
            break;
        }
        r.o.tick().await;
        after = firings_of(&r).await;
    }
    assert_eq!(after.len(), n, "the next pass screens and fires the rest");
    assert!(after.iter().all(|f| !f.screen.is_empty()), "none fired unscreened");
}

async fn firings_of(r: &Rig) -> Vec<model::Firing> {
    r.o.store
        .read(|s| {
            s.firings(
                &FiringFilter {
                    subscription: SUB.to_owned(),
                    ..FiringFilter::default()
                },
                100,
            )
        })
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_drop_is_recorded_as_an_act_of_the_decider() {
    let r = rig_with(Some(Arc::new(Retries)));
    emit_into_a_screened_start(&r, &["retry 1 succeeded", "retry 2 succeeded", "retry 3 succeeded"]).await;
    r.o.tick().await;
    let misses = r.o.store.read(|s| s.near_misses(SUB, 50)).await.unwrap();
    assert!(!misses.is_empty(), "the decider dropped what needed no chain");
    for m in &misses {
        let id = m.judgment.clone();
        let j = r.o.store.read(move |s| s.judgment(&id)).await.unwrap();
        assert_eq!(j.used, JudgmentUse::Acted, "a dropped reading's judgment is an act");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_events_of_a_signal_kept_on_this_machine_are_not_asked_about_and_fire() {
    let r = rig_with(Some(Arc::new(Retries)));
    r.o.store
        .update(|tx| {
            let es = model::EventSignal {
                id: "es_alerts".to_owned(),
                name: "alerts".to_owned(),
                kept: true,
                ..model::EventSignal::default()
            };
            tx.create_event_signal(es, &secrets::Sealed::default()).map(|_| ())
        })
        .await
        .unwrap();
    emit_into_a_screened_start(&r, &["retry 1 succeeded", "retry 2 succeeded"]).await;
    r.o.tick().await;
    let misses = r.o.store.read(|s| s.near_misses(SUB, 50)).await.unwrap();
    assert!(misses.is_empty(), "{misses:?}");
    let firings = firings_of(&r).await;
    assert_eq!(firings.len(), 2);
    assert!(firings.iter().all(|f| f.screen.is_empty()));
}

/// A chain with `pick` recorded as the screen's judgment of the reading that started it, finished by `unjudged`, with
/// `hired` cards besides its Chair: what the judgment proved.
async fn proved_after(pick: JudgmentPick, hired: usize, unjudged: Option<Unjudged>) -> Option<(Proved, OutcomeSource)> {
    let r = rig_with(None);
    let chain = r.create("r_screened", "look at the alert").await;
    let (id, now) = (chain.id.clone(), r.o.now());
    let judgment = Judgment {
        id: "ju_screen".to_owned(),
        asking: "ja_screen".to_owned(),
        question: "signal.screen@1".to_owned(),
        pick,
        p: 0.97,
        used: JudgmentUse::Acted,
        created_at: now,
        ..Judgment::default()
    };
    r.o.store
        .update(move |tx| {
            tx.put_judgments(vec![judgment])?;
            // the firing that started the chain carries the judgment, and the chain is found by it
            let mut chain = tx.chain(&id)?;
            chain.routine_id = SUB.to_owned();
            tx.put_chain(chain)?;
            tx.add_firing(&model::Firing {
                subscription_id: SUB.to_owned(),
                key: SignalKey(KEY.to_owned()),
                rev: 1,
                chain: id.clone(),
                screen: "ju_screen".to_owned(),
                ..model::Firing::default()
            })?;
            // the goal judge's card is no one hired for the work: only workers count
            let mut judge = model::Card::judge_of(model::JudgeKind::Goal);
            judge.id = "c_goal_judge".to_owned();
            judge.home = id.clone();
            judge.created_at = now;
            judge.updated_at = now;
            tx.put_card(judge)?;
            for i in 0..hired {
                let mut hired = model::Card::default();
                hired.id = format!("c_hired_{i}");
                hired.home = id.clone();
                hired.created_at = now;
                hired.updated_at = now;
                tx.put_card(hired)?;
            }
            Ok::<_, store::Error>(())
        })
        .await
        .unwrap();
    let cmd = FinishCmd {
        summary: "looked".to_owned(),
        unjudged,
        ..FinishCmd::default()
    };
    r.o.core.finish_chain(&chain.id, cmd, Records::default()).await.unwrap();
    let j = r.o.store.read(|s| s.judgment("ju_screen")).await.unwrap();
    j.outcome.map(|o| (o.proved, o.source))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_chain_that_hired_someone_proves_a_yes_right_and_a_chain_that_hired_nobody_proves_it_wrong() {
    assert_eq!(
        proved_after(JudgmentPick::Yes, 1, None).await,
        Some((Proved::Right, OutcomeSource::Chain))
    );
    assert_eq!(
        proved_after(JudgmentPick::Yes, 0, None).await,
        Some((Proved::Wrong, OutcomeSource::Chain))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_audited_no_is_proved_by_what_the_chain_it_started_did() {
    assert_eq!(
        proved_after(JudgmentPick::No, 0, None).await,
        Some((Proved::Right, OutcomeSource::Chain))
    );
    assert_eq!(
        proved_after(JudgmentPick::No, 1, None).await,
        Some((Proved::Wrong, OutcomeSource::Chain))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_chain_the_owner_ended_proves_nothing() {
    assert_eq!(
        proved_after(JudgmentPick::Yes, 0, Some(Unjudged::Owner)).await,
        Some((Proved::Unknown, OutcomeSource::Chain))
    );
}

/// How many judgments the screens have given that the code used (no shadow's).
async fn screen_judgments(r: &Rig) -> i64 {
    r.o.store
        .read(|s| s.judgment_tallies())
        .await
        .unwrap()
        .iter()
        .filter(|t| t.question == "signal.screen@1" && t.used != JudgmentUse::Shadow)
        .map(|t| t.judgments)
        .sum()
}

/// Rewinds the cursor of the screened start to the head of the log, with its screen worded as screen.
async fn rewind_with(r: &Rig, screen: &str) {
    let screen = screen.to_owned();
    r.o.store
        .update(move |tx| {
            let mut sub = tx.subscription(SUB)?;
            sub.screen = screen;
            sub.seen_seq = 0;
            tx.put_subscription(sub).map(|_| ())
        })
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_screen_the_assistant_corrected_asks_again_about_what_the_old_one_judged() {
    let r = rig_with(Some(Arc::new(Retries)));
    emit_into_a_screened_start(&r, &["retry 1 succeeded", "retry 2 succeeded", "pipeline failed"]).await;
    r.o.tick().await;
    assert_eq!(screen_judgments(&r).await, 3, "each event is judged once");

    rewind_with(&r, SCREEN).await;
    r.o.tick().await;
    assert_eq!(
        screen_judgments(&r).await,
        3,
        "the same screen reads the first answers instead of asking again"
    );

    rewind_with(&r, "The alert needs a person, and nothing already handled it.").await;
    r.o.tick().await;
    assert_eq!(
        screen_judgments(&r).await,
        6,
        "a corrected screen asks about each event under its new words"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_screen_on_a_batch_is_refused() {
    let r = rig_with(None);
    let sub = Subscription {
        id: SUB.to_owned(),
        r#match: SignalPattern(KEY.to_owned()),
        screen: SCREEN.to_owned(),
        flow: model::Flow {
            policy: model::FlowPolicy::Batch,
            ..model::Flow::default()
        },
        action: SubscriptionAction {
            kind: ActionKind::Start,
            start: Some(StartSpec {
                title: "Alert".to_owned(),
                goal: "Find out why.".to_owned(),
                ..StartSpec::default()
            }),
            ..SubscriptionAction::default()
        },
        ..Subscription::default()
    };
    let (now, from) = (r.o.now(), Start::default());
    let refused = r.o.store.update(move |tx| subscribe(tx, sub, &from, now)).await;
    assert!(
        refused.is_err(),
        "a batch fires on many readings, and a screen judges one"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_assistant_keeps_an_event_signal_on_this_machine_and_a_later_update_leaves_it_so() {
    let r = rig_with(None);
    r.o.core.ensure_assistant().await.unwrap();
    let card = r.o.core.assistant_card().await.unwrap().id;
    let tools = r.o.tools_for(&card).await.unwrap();
    let call = |name: &'static str, args: Value| {
        let tools = &tools;
        async move {
            let args = args.as_object().cloned().unwrap();
            tools.call(name, Some(args)).await.unwrap().unwrap()
        }
    };
    call("create_event_signal", json!({"name": "alerts"})).await;
    let kept = |r: &Rig| {
        let store = r.o.store.clone();
        async move {
            let all = store.read(|s| s.event_signals()).await.unwrap();
            all.into_iter().find(|es| es.name == "alerts").unwrap()
        }
    };
    let es = kept(&r).await;
    assert!(!es.kept);
    call("update_event_signal", json!({"id": es.id, "kept": true})).await;
    assert!(kept(&r).await.kept);
    call("update_event_signal", json!({"id": es.id, "note": "Alerts of staging"})).await;
    assert!(
        kept(&r).await.kept,
        "an update that does not say kept leaves it as it was"
    );
    call("update_event_signal", json!({"id": es.id, "kept": false})).await;
    assert!(!kept(&r).await.kept);
}
