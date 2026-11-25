//! Admission's rails (ADR-0115), driven through the pure rules the scheduler calls each Tick.

use std::collections::HashMap;

use chains::admit::{Outcome, capacity, held_by_key_result, queue_of, stall_on, stalled};
use model::{Chain, ChainStatus, KeyResult, Landing, LandingState, Lift, ReturnReason, RevertCause, Serves, Time};
use time::{Duration, OffsetDateTime};

// The limit is measured, not set: it grows by one a look while a start waits and nothing presses, halves under pressure,
// and never falls below what runs, so it holds what would start and stops nothing.
#[test]
fn the_limit_grows_while_a_start_waits_halves_under_pressure_and_never_stops_what_runs() {
    assert_eq!(capacity(4, 4, true, false), 5, "a start waits, nothing presses");
    assert_eq!(capacity(4, 2, false, false), 4, "no start waits on it");
    assert_eq!(capacity(8, 2, true, true), 4, "pressure halves it");
    assert_eq!(capacity(8, 6, true, true), 6, "halving never goes below what runs");
    assert_eq!(capacity(1, 0, false, true), 1, "never below 1");
}

fn at(minutes: i64) -> Time {
    Time(OffsetDateTime::UNIX_EPOCH.saturating_add(Duration::days(20_000).saturating_add(Duration::minutes(minutes))))
}

fn serving(objective: &str, kr: &str) -> Serves {
    Serves {
        objective: objective.to_owned(),
        key_result: kr.to_owned(),
    }
}

fn chain(id: &str, status: ChainStatus, serves: Serves, created: i64) -> Chain {
    let mut c = Chain::default();
    (c.id, c.title, c.serves, c.created_at) = (id.to_owned(), id.to_owned(), Some(serves), at(created));
    c.by = Some(model::by_agent("c_proposer"));
    c.set_status(status);
    c
}

fn outcome(id: &str, created: i64, progressed: bool) -> Outcome {
    Outcome {
        chain: id.to_owned(),
        title: id.to_owned(),
        serves: serving("ob_1", "kr_a"),
        created_at: at(created),
        progressed,
    }
}

// The progress rail: a key result whose last 3 finished chains made no progress takes no new chain, one that progressed
// among them clears it, and a lift counts only the chains after it.
#[test]
fn a_key_result_stalls_after_three_chains_without_progress_until_lifted() {
    let kr = KeyResult {
        id: "kr_a".to_owned(),
        ..KeyResult::default()
    };
    let three = [
        outcome("ch_1", 1, false),
        outcome("ch_2", 2, false),
        outcome("ch_3", 3, false),
    ];
    assert!(
        stalled("ob_1", &kr, &[], &three[..2]).is_none(),
        "two chains are not three"
    );
    let why = stalled("ob_1", &kr, &[], &three).expect("three chains without progress stall it");
    assert!(
        why.contains("ch_1") && why.contains("ch_3"),
        "it names the chains: {why}"
    );
    assert!(
        stalled("ob_2", &kr, &[], &three).is_none(),
        "another objective's key result of that id is not stalled"
    );

    let mut older_progress = vec![outcome("ch_0", 0, true)];
    older_progress.extend(three.iter().cloned());
    assert!(
        stalled("ob_1", &kr, &[], &older_progress).is_some(),
        "only the last 3 count"
    );
    let mut progressed = three.to_vec();
    progressed[1].progressed = true;
    assert!(
        stalled("ob_1", &kr, &[], &progressed).is_none(),
        "one of the last 3 progressed"
    );

    let lifted = KeyResult {
        lifted: Some(Lift {
            at: at(2),
            ..Lift::default()
        }),
        ..kr
    };
    assert!(
        stalled("ob_1", &lifted, &[], &three).is_none(),
        "a lift forgets the chains before it"
    );
}

// A key result's chains count for every key result it refines: a child's progress is progress on its parent, three
// children's chains without progress stall the parent, and a fresh child of a stalled parent is held with it.
#[test]
fn a_stall_counts_what_refines_a_key_result_and_holds_a_fresh_child() {
    let kr = |id: &str, parent: Option<&str>| KeyResult {
        id: id.to_owned(),
        parent: parent.map(str::to_owned),
        ..KeyResult::default()
    };
    let tree = [kr("kr_p", None), kr("kr_a", Some("kr_p")), kr("kr_new", Some("kr_p"))];
    let three = [
        outcome("ch_1", 1, false),
        outcome("ch_2", 2, false),
        outcome("ch_3", 3, false),
    ];
    assert!(
        stalled("ob_1", &tree[0], &tree, &three).is_some(),
        "three chains under the parent without progress stall it"
    );
    let why = stall_on("ob_1", &tree, "kr_new", &three).expect("a fresh child of a stalled parent is held");
    assert!(why.contains("kr_p"), "it names the stalled parent: {why}");
    let mut progressed = three.to_vec();
    progressed[2].progressed = true;
    assert!(
        stall_on("ob_1", &tree, "kr_new", &progressed).is_none(),
        "a child's progress is progress on its parent"
    );
}

// Shares by key result: each served key result runs one chain before any runs two, then the soonest horizon goes first,
// then the oldest; a chain past its key result's share waits, a stalled key result's chain waits, and a chain the owner
// asked for is never held by either.
#[test]
fn admission_shares_the_limit_by_key_result_and_never_holds_the_owners_ask() {
    let busy = chain("ch_run", ChainStatus::Running, serving("ob_late", "kr_busy"), 0);
    let second = chain("ch_second", ChainStatus::Queued, serving("ob_late", "kr_busy"), 1);
    let late = chain("ch_late", ChainStatus::Queued, serving("ob_late", "kr_free"), 2);
    let soon = chain("ch_soon", ChainStatus::Queued, serving("ob_soon", "kr_x"), 3);
    let none = chain("ch_none", ChainStatus::Queued, serving("o_task", "k_task"), 4);
    let chains = vec![busy, second.clone(), late, soon, none];
    let horizons = HashMap::from([("ob_soon".to_owned(), at(100)), ("ob_late".to_owned(), at(900))]);
    let order: Vec<&str> = queue_of(&chains, &horizons).iter().map(|c| c.id.as_str()).collect();
    assert_eq!(order, ["ch_soon", "ch_late", "ch_none", "ch_second"]);

    let held = held_by_key_result(&second, &chains, &[], "");
    assert!(!held.is_empty(), "the key result's one place is taken: {held}");
    let free = &chains[2];
    assert_eq!(
        held_by_key_result(free, &chains, &[], ""),
        "",
        "a key result with nothing running takes a chain"
    );
    let stall = held_by_key_result(free, &chains, &[], "key result kr_free is stalled");
    assert!(
        stall.contains("stalled"),
        "a stalled key result holds its starts: {stall}"
    );

    let mut asked = second;
    asked.by = Some(model::by_owner("owner", "web"));
    assert_eq!(
        held_by_key_result(&asked, &chains, &[], "stalled"),
        "",
        "the owner's ask is never held"
    );
}

fn landing(chain: &str, state: LandingState, minute: i64) -> Landing {
    Landing {
        id: format!("ln_{chain}_{minute}"),
        chain_id: chain.to_owned(),
        landed_at: Some(at(minute)),
        reverted_at: Some(at(minute)),
        updated_at: at(minute),
        state,
        ..Landing::default()
    }
}

fn reverted() -> LandingState {
    LandingState::Reverted {
        revert: "ln_revert".into(),
        cause: RevertCause::Pipeline,
        by: model::by_remit("landing"),
        why: "the pipeline failed".into(),
    }
}

// A key result's share grows by one for each landing of its work that holds on its base and halves for each taken back
// or returned for what the queue found, so its second chain starts once its work has landed twice; and a key result
// whose landings already wait in the merge queue as many as its share takes no new start (back-pressure).
#[test]
fn a_share_grows_on_held_landings_halves_on_reverts_and_waits_on_the_queue() {
    let busy = chain("ch_run", ChainStatus::Running, serving("ob_1", "kr_a"), 0);
    let second = chain("ch_second", ChainStatus::Queued, serving("ob_1", "kr_a"), 1);
    let chains = vec![busy, second.clone()];
    let held = |landings: &[Landing]| held_by_key_result(&second, &chains, landings, "");

    assert!(!held(&[]).is_empty(), "a share of 1 is taken by the chain running");
    let landed = [landing("ch_run", LandingState::Landed, 1)];
    assert_eq!(held(&landed), "", "one landing that held makes the share 2");

    let taken = [
        landing("ch_run", LandingState::Landed, 1),
        landing("ch_run", reverted(), 2),
    ];
    assert!(!held(&taken).is_empty(), "a revert halves it back to 1");
    let returned = [
        landing("ch_run", LandingState::Landed, 1),
        landing(
            "ch_run",
            LandingState::Returned {
                reason: ReturnReason::Conflict,
                output: String::new(),
            },
            2,
        ),
    ];
    assert!(!held(&returned).is_empty(), "a conflict the queue found halves it too");
    let replaced = [
        landing("ch_run", LandingState::Landed, 1),
        landing(
            "ch_run",
            LandingState::Returned {
                reason: ReturnReason::Replaced,
                output: String::new(),
            },
            2,
        ),
    ];
    assert_eq!(held(&replaced), "", "a newer version says nothing of the work");

    let waits = [
        landing("ch_run", LandingState::Landed, 1),
        landing("ch_run", LandingState::Ready, 2),
        landing("ch_run", LandingState::Checking, 3),
    ];
    let why = held(&waits);
    assert!(
        why.contains("merge queue"),
        "two waiting in the queue fill a share of 2: {why}"
    );
}
