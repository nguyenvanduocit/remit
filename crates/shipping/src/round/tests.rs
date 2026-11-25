//! The round's rules as tables: when a round is judged, what a pick does to the versions contending in it, and where an
//! attempt's approved version goes.

use model::{Chain, ChainStatus, Landing, LandingState, ReturnReason, Round, RoundState, Time};

use super::*;

fn at(minute: i64) -> Time {
    Time(time::OffsetDateTime::UNIX_EPOCH.saturating_add(time::Duration::minutes(minute)))
}

fn attempt(id: &str, minute: i64, status: ChainStatus) -> Chain {
    let mut c = Chain::default();
    id.clone_into(&mut c.id);
    c.attempt = Some("ro_1".to_owned());
    c.created_at = at(minute);
    c.set_status(status);
    c
}

fn version(id: &str, chain: &str, state: LandingState) -> Landing {
    Landing {
        id: id.to_owned(),
        chain_id: chain.to_owned(),
        state,
        ..Landing::default()
    }
}

fn round(state: RoundState) -> Round {
    Round {
        id: "ro_1".to_owned(),
        number: 2,
        state,
        ..Round::default()
    }
}

// A round waits while an attempt neither contends nor finished, is judged once each does one or the other (the first
// opened first, an attempt that finished without a version left out), and ends when none has a version.
#[test]
fn a_round_is_judged_once_every_attempt_contends_or_finished() {
    let (a, b, c) = (
        attempt("ch_a", 1, ChainStatus::Waiting),
        attempt("ch_b", 0, ChainStatus::Running),
        attempt("ch_c", 2, ChainStatus::Done),
    );
    let mut landings = vec![
        version("ln_a", "ch_a", LandingState::Contending),
        version("ln_b", "ch_b", LandingState::Judging),
    ];
    let all = [a, b, c.clone()];
    assert_eq!(
        standing(&all, &landings),
        Standing::Works,
        "ch_b's version is still judged"
    );
    landings[1].state = LandingState::Contending;
    assert_eq!(
        standing(&all, &landings),
        Standing::Judge(vec!["ch_b".to_owned(), "ch_a".to_owned()])
    );
    assert_eq!(
        standing(&[c], &[]),
        Standing::Empty,
        "every attempt finished without one"
    );
}

// The pick's version enters its queue at the pick; every other contending version goes back not picked, saying why; a
// round that ends without a pick sends them all back. What does not contend is left as it is.
#[test]
fn a_pick_queues_its_attempt_and_returns_the_others() {
    let landings = [
        version("ln_a", "ch_a", LandingState::Contending),
        version("ln_b", "ch_b", LandingState::Contending),
        version("ln_old", "ch_b", LandingState::Landed),
    ];
    let moved = picked(&landings, Some("ch_b"), "it passes the check", at(9));
    assert_eq!(moved.len(), 2, "{moved:?}");
    let b = moved.iter().find(|l| l.id == "ln_b").unwrap();
    assert_eq!((&b.state, b.ready_at), (&LandingState::Ready, Some(at(9))));
    let a = moved.iter().find(|l| l.id == "ln_a").unwrap();
    assert_eq!(
        a.state,
        LandingState::Returned {
            reason: ReturnReason::NotPicked,
            output: "it passes the check".to_owned()
        }
    );
    let none = picked(&landings, None, "no pick", at(9));
    assert!(
        none.iter().all(|l| matches!(
            l.state,
            LandingState::Returned {
                reason: ReturnReason::NotPicked,
                ..
            }
        )),
        "{none:?}"
    );
}

// An approved version enters its queue by itself only when its chain is no attempt; an attempt's contends while its round
// is unsettled, and goes back once the round picked or ended.
#[test]
fn an_attempts_approved_version_never_enters_the_queue_by_itself() {
    assert_eq!(approved_state(None), LandingState::Ready);
    for unsettled in [
        RoundState::Open,
        RoundState::Judging {
            judge: "c_j".to_owned(),
        },
    ] {
        assert_eq!(approved_state(Some(&round(unsettled))), LandingState::Contending);
    }
    let settled = round(RoundState::Picked {
        judge: "c_j".to_owned(),
        attempt: "ch_a".to_owned(),
        why: "it passes".to_owned(),
        overturned: None,
    });
    assert!(
        matches!(
            approved_state(Some(&settled)),
            LandingState::Returned { reason: ReturnReason::NotPicked, ref output } if output.contains("round 2")
        ),
        "{:?}",
        approved_state(Some(&settled))
    );
}
