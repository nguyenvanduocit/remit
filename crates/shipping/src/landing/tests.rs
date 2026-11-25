//! The queue's rule as tables: what goes into a batch, how a red batch is bisected, and what a pipeline's word does.

use model::{Checks, ChecksState, Landing, LandingState, LandingVersion, StagePlan, Time};

use super::*;

fn at(minute: i64) -> Time {
    Time(time::OffsetDateTime::UNIX_EPOCH.saturating_add(time::Duration::minutes(minute)))
}

fn ready(id: &str, minute: i64) -> Landing {
    Landing {
        id: id.to_owned(),
        state: LandingState::Ready,
        ready_at: Some(at(minute)),
        created_at: at(minute),
        ..Landing::default()
    }
}

fn landed(id: &str, minute: i64) -> Landing {
    Landing {
        id: id.to_owned(),
        state: LandingState::Landed,
        landed_at: Some(at(minute)),
        ..Landing::default()
    }
}

fn revert(id: &str, of: &str, minute: i64) -> Landing {
    Landing {
        reverts: Some(model::Revert {
            landing: of.to_owned(),
            cause: RevertCause::Pipeline,
            by: model::by_remit("queue"),
            why: String::new(),
        }),
        ..ready(id, minute)
    }
}

fn returned() -> LandingState {
    LandingState::Returned {
        reason: model::ReturnReason::Checks,
        output: String::new(),
    }
}

fn staged(id: &str, minute: i64) -> Landing {
    Landing {
        version: LandingVersion {
            plan: Some(StagePlan::default()),
            ..LandingVersion::default()
        },
        ..ready(id, minute)
    }
}

fn ids(batch: &[Landing]) -> Vec<&str> {
    batch.iter().map(|l| l.id.as_str()).collect()
}

/// A case of `next_batch`: its name, the queue, k, the held stages and the batch it gives.
type BatchCase = (&'static str, Vec<Landing>, usize, Vec<&'static str>, Vec<&'static str>);

#[test]
fn a_batch_takes_the_ready_in_order_reverts_first_and_a_stage_alone() {
    let judging = Landing {
        state: LandingState::Judging,
        ..ready("judging", 0)
    };
    let cases: Vec<BatchCase> = vec![
        (
            "nothing ready",
            vec![judging.clone(), landed("old", 0)],
            4,
            vec![],
            vec![],
        ),
        (
            "the oldest ready first, up to k",
            vec![ready("b", 2), ready("a", 1), ready("c", 3), judging],
            2,
            vec![],
            vec!["a", "b"],
        ),
        (
            "k of zero is one",
            vec![ready("a", 1), ready("b", 2)],
            0,
            vec![],
            vec!["a"],
        ),
        (
            "reverts first, the newest target first, and only with reverts",
            vec![
                ready("a", 1),
                landed("x", 10),
                landed("y", 20),
                revert("rx", "x", 30),
                revert("ry", "y", 31),
            ],
            8,
            vec![],
            vec!["ry", "rx"],
        ),
        (
            "a stage goes alone",
            vec![staged("s", 1), ready("a", 2)],
            8,
            vec![],
            vec!["s"],
        ),
        (
            "a stage ends the batch before it",
            vec![ready("a", 1), staged("s", 2), ready("b", 3)],
            8,
            vec![],
            vec!["a"],
        ),
        (
            "a held stage waits and the queue goes on past it",
            vec![staged("s", 1), ready("a", 2), ready("b", 3)],
            8,
            vec!["s"],
            vec!["a", "b"],
        ),
    ];
    for (name, queue, k, held, want) in cases {
        let held: Vec<String> = held.into_iter().map(str::to_owned).collect();
        assert_eq!(ids(&next_batch(&queue, k, &held)), want, "{name}");
    }
}

#[test]
fn a_red_batch_is_bisected_down_to_the_landing_that_broke_it() {
    // eight ready, the fifth breaks the build: halving after each red batch and growing after each green one lands the
    // seven and finds the fifth alone
    let mut queue: Vec<Landing> = (1..=8).map(|i| ready(&format!("l{i}"), i)).collect();
    let mut k = MAX_BATCH;
    let mut back = Vec::new();
    for _ in 0..32 {
        let batch = next_batch(&queue, k, &[]);
        if batch.is_empty() {
            break;
        }
        let red = batch.iter().any(|l| l.id == "l5");
        for l in queue.iter_mut().filter(|l| batch.iter().any(|b| b.id == l.id)) {
            l.state = match (red, batch.len()) {
                (false, _) => LandingState::Landed,
                (true, 1) => returned(),
                (true, _) => LandingState::Ready,
            };
            if l.state == returned() {
                back.push(l.id.clone());
            }
        }
        k = if red { halved(k) } else { grown(k) };
    }
    assert_eq!(back, vec!["l5"]);
    let landed: Vec<&str> = queue
        .iter()
        .filter(|l| l.state == LandingState::Landed)
        .map(|l| l.id.as_str())
        .collect();
    assert_eq!(landed, vec!["l1", "l2", "l3", "l4", "l6", "l7", "l8"]);
    assert_eq!((grown(MAX_BATCH), halved(1)), (MAX_BATCH, 1));
}

#[test]
fn a_push_is_reverted_only_when_it_broke_a_green_base() {
    let c = |state| {
        Some(Checks {
            state,
            failed: String::new(),
        })
    };
    let (pass, fail, pending) = (c(ChecksState::Passed), c(ChecksState::Failed), c(ChecksState::Pending));
    let cases = [
        ("passed", pass.clone(), pass.clone(), Verdict::Stands),
        ("failed on a green base", fail.clone(), pass.clone(), Verdict::Revert),
        ("failed on a red base", fail.clone(), fail.clone(), Verdict::Stands),
        (
            "failed, the base not settled",
            fail.clone(),
            pending.clone(),
            Verdict::Wait,
        ),
        ("failed, the base never ran", fail, None, Verdict::Wait),
        ("still running", pending, pass, Verdict::Wait),
        ("not run yet", None, None, Verdict::Wait),
    ];
    for (name, push, onto, want) in cases {
        assert_eq!(verdict_on(push.as_ref(), onto.as_ref()), want, "{name}");
    }
}

#[test]
fn a_revert_takes_back_only_what_is_on_the_base_and_lands_first() {
    let mut target = landed("t", 5);
    target.version.title = "feat: x".to_owned();
    target.commit = Some("abc".to_owned());
    assert!(revertible(&target, &[target.clone()]));
    let r = revert_of(
        &target,
        "r",
        RevertCause::Pipeline,
        model::by_remit("queue"),
        "the pipeline failed",
        at(9),
    );
    assert_eq!(
        (r.reverted_one(), r.state.clone(), r.version.title.as_str()),
        (Some("t"), LandingState::Ready, "Revert \"feat: x\"")
    );
    let queue = vec![target.clone(), r.clone()];
    assert!(!revertible(&r, &queue), "a revert is never reverted");
    assert!(!revertible(&target, &queue), "one revert at a time");
    assert!(target.on_base(), "a change stays on the base until its revert lands");
    let gone = taken_back(target, &r, at(10));
    assert!(!gone.on_base());
    assert!(
        matches!(&gone.state, LandingState::Reverted { revert, cause: RevertCause::Pipeline, .. } if revert == "r"),
        "it says what took it back: {:?}",
        gone.state
    );
    let queue = vec![ready("a", 1), r];
    assert_eq!(ids(&next_batch(&queue, 8, &[])), vec!["r"]);
}
