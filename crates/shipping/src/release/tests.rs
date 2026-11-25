//! The release rule as tables: what a procedure declares, which release is next, and what a step's exit says.

use model::{Actor, Checks, ChecksState, Landing, LandingState, Release, ReleaseState, Revert, RevertCause, Time};

use super::*;

fn at(minute: i64) -> Time {
    Time(time::OffsetDateTime::UNIX_EPOCH.saturating_add(time::Duration::minutes(minute)))
}

/// A landing on its base at minute whose push the forge's checks passed.
fn landed(id: &str, minute: i64) -> Landing {
    Landing {
        id: id.to_owned(),
        state: LandingState::Landed,
        commit: Some(format!("c_{id}")),
        landed_at: Some(at(minute)),
        checks: Some(Checks {
            state: ChecksState::Passed,
            failed: String::new(),
        }),
        ..Landing::default()
    }
}

fn release(id: &str, top: &str, state: ReleaseState) -> Release {
    Release {
        id: id.to_owned(),
        top: top.to_owned(),
        state,
        ..Release::default()
    }
}

fn carried_by(mut l: Landing, release: &str) -> Landing {
    l.release = Some(release.to_owned());
    l
}

fn tops(d: Option<Due<'_>>) -> Option<(String, Vec<String>)> {
    d.map(|d| (d.top.id.clone(), d.carried.iter().map(|l| l.id.clone()).collect()))
}

#[test]
fn a_procedure_is_its_executables_in_name_order() {
    let entries = [
        (".remit/release/20-publish".to_owned(), true),
        (".remit/release/10-build".to_owned(), true),
        (".remit/release/README.md".to_owned(), false),
        (".remit/release/connections".to_owned(), false),
        (".remit/release/back/10-yank".to_owned(), true),
        (".remit/release/lib/helper".to_owned(), true),
    ];
    let p = procedure(
        &entries,
        "http/crates-io  # the registry\nhttp/npm\n",
        "fact:remit.health 60 value.healthy == false",
    )
    .unwrap()
    .unwrap();
    assert_eq!(p.steps, [".remit/release/10-build", ".remit/release/20-publish"]);
    assert_eq!(p.back, [".remit/release/back/10-yank"]);
    assert_eq!(
        p.connections.iter().map(Connection::place).collect::<Vec<_>>(),
        ["http/crates-io", "http/npm"]
    );
    let g = p.guard.unwrap();
    assert_eq!(g.signal.as_str(), "fact:remit.health");
    assert_eq!(g.bake.0, 3_600_000_000_000, "an hour, in nanoseconds");
    assert_eq!(procedure(&[], "", "").unwrap(), None, "no step: no release");
    assert!(
        procedure(&entries, "crates-io\n", "").is_err(),
        "a connection names its source"
    );
    assert!(
        procedure(&entries, "", "fact:x soon value.ok").is_err(),
        "a bake is minutes"
    );
}

// A rail of ADR-0116: one release of a repository at a time, none while one releases or bakes.
#[test]
fn no_release_starts_while_one_bakes() {
    let all = [carried_by(landed("a", 1), "r1"), landed("b", 2)];
    for state in [ReleaseState::Releasing, ReleaseState::Baking] {
        let releases = [release("r1", "a", state)];
        assert_eq!(tops(next_release(&all, &releases, 1, false, at(100))), None);
    }
    let releases = [release("r1", "a", ReleaseState::Live)];
    assert_eq!(
        tops(next_release(&all, &releases, 1, false, at(100))),
        Some(("b".to_owned(), vec!["b".to_owned()])),
        "once it is live the next carries what landed since"
    );
}

#[test]
fn a_release_is_at_the_kth_oldest_settled_landing() {
    let mut pending = landed("c", 3);
    pending.checks = Some(Checks {
        state: ChecksState::Pending,
        failed: String::new(),
    });
    let all = [
        carried_by(landed("a", 1), "r1"),
        landed("b", 2),
        pending,
        landed("d", 4),
    ];
    let releases = [release("r1", "a", ReleaseState::Live)];
    let next = |k| tops(next_release(&all, &releases, k, false, at(100))).map(|(t, _)| t);
    assert_eq!(next(1).as_deref(), Some("b"));
    assert_eq!(
        next(2).as_deref(),
        Some("b"),
        "the second is pending: the newest settled below it"
    );
    assert_eq!(next(3).as_deref(), Some("d"));
    assert_eq!(next(8).as_deref(), Some("d"));
    assert_eq!(
        tops(next_release(&all[1..], &[], 1, false, at(100)))
            .map(|(t, _)| t)
            .as_deref(),
        Some("d"),
        "a repository's first release is at its newest landing"
    );
    let mut quiet = landed("e", 98);
    (quiet.checks, quiet.base_checks) = (None, None);
    assert_eq!(
        tops(next_release(std::slice::from_ref(&quiet), &[], 1, false, at(100))),
        None
    );
    assert!(
        tops(next_release(std::slice::from_ref(&quiet), &[], 1, false, at(110))).is_some(),
        "a forge that said nothing for ten minutes runs no pipeline"
    );
}

#[test]
fn a_release_with_nothing_new_is_passed_over_until_more_lands() {
    let all = [landed("a", 1), landed("b", 2)];
    let releases = [release("r1", "b", ReleaseState::Unchanged)];
    assert_eq!(tops(next_release(&all, &releases, 1, false, at(100))), None);
    let more = [landed("a", 1), landed("b", 2), landed("c", 3)];
    assert_eq!(
        tops(next_release(&more, &releases, 1, false, at(100))),
        Some(("c".to_owned(), vec!["a".to_owned(), "b".to_owned(), "c".to_owned()])),
        "the next release carries what waited"
    );
}

// The k counts from where the release may start: past what a release that published nothing looked at, it carries k.
#[test]
fn a_release_past_one_that_published_nothing_carries_k_landings() {
    let all = [landed("a", 1), landed("b", 2), landed("c", 3), landed("d", 4)];
    let releases = [release("r1", "a", ReleaseState::Unchanged)];
    assert_eq!(
        tops(next_release(&all, &releases, 2, false, at(100))).map(|(t, _)| t),
        Some("c".to_owned()),
        "b and c are the two past a"
    );
}

// What a release taken back carried is released again only once every revert of it landed: the release forward.
#[test]
fn a_release_taken_back_waits_for_its_reverts_then_releases_them() {
    let taken = ReleaseState::TakenBack {
        by: Actor::default(),
        why: "breach".to_owned(),
    };
    let releases = [release("r1", "a", taken)];
    let revert = |id: &str, of: &str, minute| Landing {
        reverts: Some(Revert {
            landing: of.to_owned(),
            cause: RevertCause::Guardrail,
            by: Actor::default(),
            why: String::new(),
        }),
        ..landed(id, minute)
    };
    let waiting = [carried_by(landed("a", 1), "r1"), landed("b", 2)];
    assert_eq!(
        tops(next_release(&waiting, &releases, 1, false, at(100))),
        None,
        "a is still on the base"
    );
    let mut gone = carried_by(landed("a", 1), "r1");
    gone.state = LandingState::Reverted {
        revert: "ra".to_owned(),
        cause: RevertCause::Guardrail,
        by: Actor::default(),
        why: String::new(),
    };
    let after = [gone, landed("b", 2), revert("ra", "a", 3)];
    assert_eq!(
        tops(next_release(&after, &releases, 1, false, at(100))),
        Some(("ra".to_owned(), vec!["b".to_owned(), "ra".to_owned()])),
        "the release forward carries the revert"
    );
}

// Remit's own procedure (ADR-0116) is one the runner reads: its step, its connection and its guard on the server's health.
#[test]
fn remits_own_procedure_parses() {
    let entries = [(".remit/release/10-await-workflow-release".to_owned(), true)];
    let p = procedure(
        &entries,
        include_str!("../../../../.remit/release/connections"),
        include_str!("../../../../.remit/release/guard"),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        p.connections.iter().map(Connection::place).collect::<Vec<_>>(),
        ["github/default"]
    );
    assert_eq!(p.guard.unwrap().signal.as_str(), "fact:remit.health");
}

#[test]
fn a_step_retries_only_when_the_outside_was_unreachable() {
    let table = [
        (Some(0), false, StepOutcome::Passed),
        (Some(TEMPFAIL), false, StepOutcome::Retry),
        (None, false, StepOutcome::Retry),
        (Some(1), false, StepOutcome::Breach),
        (Some(0), true, StepOutcome::Breach),
    ];
    for (exit, timed_out, want) in table {
        assert_eq!(step_outcome(exit, timed_out), want, "{exit:?} {timed_out}");
    }
}

#[test]
fn the_out_file_says_what_was_released() {
    let (out, note) = out_of(Some(r#"{"released": false}"#));
    assert!(!out.released && note.is_none());
    let (out, _) = out_of(Some(
        r#"{"released": true, "version": "0.4.0", "links": ["https://crates.io/crates/pixfx"]}"#,
    ));
    assert_eq!((out.version.as_str(), out.links.len()), ("0.4.0", 1));
    assert!(out_of(None).0.released, "no file: the steps passed, so it released");
    assert!(
        out_of(Some("not json")).1.is_some(),
        "a file that is not the contract is said so"
    );
}
