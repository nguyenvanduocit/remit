//! The check rail (ADR-0068): verification is the product. A chain's key result has a check that can fail, declared
//! before the work (state_check, or the owner's objective), and the chain does not finish, nor land a stage, without it.
//! A result that ships also carries its telemetry, its incident plan and a watch on that telemetry. `check_gaps` is the
//! rule; this module reads what it rules on and runs a command check at the finish.

use model::{Chain, Check, CheckKind, Decision, DecisionKind, KeyResult, Time};
use telemetry::attr;
use tracing::Instrument as _;

use crate::Shipping;
use crate::land::{not_open_text, work_named};
use crate::repochecks::{JailRun, command_check};
use crate::state_check::check_target;
use chains::chain_core::optional;
use chains::outbox::by_remit;
use chains::wait::WakeSubs;
use chains::watch::{WATCH_SUBSCRIBED_PREFIX, WATCH_WOKE_PREFIX};
use chains::{Error, Result};

/// The words of a command check's run, written on the chain as a decision: the last of them is the check's last result.
pub(crate) const CHECK_PASSED_PREFIX: &str = "The check passed: ";
pub(crate) const CHECK_FAILED_PREFIX: &str = "The check failed: ";

/// What the chain's watch decisions and its open watches (held) say of the signal key at now (watch.rs writes their
/// words). Pure.
pub(crate) fn telemetry_watch_of(decisions: &[Decision], held: &WakeSubs, key: &str, now: Time) -> TelemetryWatch {
    let mut w = TelemetryWatch::default();
    if held.open_at(now).iter().any(|s| s.sub.r#match.as_str() == key) {
        (w.open, w.watched) = (true, true);
    }
    let (subscribed, woke) = (
        format!("{WATCH_SUBSCRIBED_PREFIX}{key} "),
        format!("{WATCH_WOKE_PREFIX}{key} "),
    );
    for d in decisions {
        if d.kind == DecisionKind::Subscribe && d.text.starts_with(&subscribed) {
            (w.watched, w.fired) = (true, false);
        } else if d.kind == DecisionKind::Wake && d.text.starts_with(&woke) {
            w.fired = true;
        }
    }
    w
}

impl Shipping {
    /// The key result chain ch serves, and whether it is its own direct request's (task) rather than the owner's.
    pub(crate) async fn served_check(&self, ch: &Chain) -> Result<(KeyResult, bool)> {
        let Some(serves) = ch.serves.clone() else {
            return Err(Error::refused(format!("chain {} serves no key result", ch.id)));
        };
        let (key, kr_id) = (serves.objective.clone(), serves.key_result.clone());
        let (obj, kr) = self
            .store
            .read(move |r| Ok::<_, Error>((r.objective(&key)?, optional(r.key_result(&key, &kr_id))?)))
            .await
            .map_err(|err| Error::refused(format!("the objective chain {} serves: {err}", ch.id)))?;
        kr.map(|kr| (kr, obj.task)).ok_or_else(|| {
            Error::refused(format!(
                "objective {} has no key result {}",
                serves.objective, serves.key_result
            ))
        })
    }

    /// The key result chain ch serves and why the chain may not pass door on its check, with what the Chair does about
    /// it: "" when it may.
    pub(crate) async fn check_refusal(&self, ch: &Chain, door: CheckDoor) -> Result<(KeyResult, String)> {
        let (kr, task) = self.served_check(ch).await?;
        let id = ch.id.clone();
        let decisions = self.store.read(move |r| r.decisions(&id)).await?;
        let held = self.waits_of(&ch.id).await?;
        let seen = kr.check.as_ref().map_or("", Check::seen);
        let watch = telemetry_watch_of(&decisions, &held, seen, self.now());
        let id = ch.id.clone();
        let landings = self.store.read(move |r| r.chain_landings(&id)).await?;
        let ships = landings.iter().any(|l| l.version.plan.is_some());
        let gaps = check_gaps(kr.check.as_ref(), ships, watch, door);
        if gaps.is_empty() {
            return Ok((kr, String::new()));
        }
        let next = if task {
            "Declare what is missing with state_check (all of the check at once: it replaces the one stated), and subscribe \
             the watch it names."
                .to_owned()
        } else {
            format!(
                "The check is on the owner's objective: ask with ask_owner for key result {} to be amended with what is \
                 missing (the Assistant decides it for him), and subscribe for the answer. A watch is yours to subscribe.",
                kr.id
            )
        };
        let why = format!(
            "Key result {} cannot pass on its check yet:\n- {}\n{next}",
            kr.id,
            gaps.join("\n- ")
        );
        Ok((kr, why))
    }

    /// Runs the command check c of chain ch in the worktree it names, in the jail run, and records the run as a
    /// decision: why the chain may not finish when it fails, "" when it passes. A check that cannot run fails: a finish
    /// never passes for want of a place to run.
    pub(crate) async fn command_refusal(&self, run: &JailRun, ch: &Chain, c: &Check) -> String {
        let ck = match work_named(&ch.repos, &c.repo).and_then(|i| ch.repos.get(i)) {
            Some(rw) => {
                command_check(run, &rw.path, &c.name)
                    .instrument(tracing::info_span!(attr::CHECKS_SPAN, { attr::CHAIN_ID } = %ch.id))
                    .await
            }
            None => chains::git::Check {
                command: c.name.clone(),
                output: not_open_text(&ch.repos, &c.repo),
                ..chains::git::Check::default()
            },
        };
        let prefix = if ck.ok {
            CHECK_PASSED_PREFIX
        } else {
            CHECK_FAILED_PREFIX
        };
        self.decide(
            &ch.id,
            DecisionKind::Goal,
            &format!("{prefix}{}.", check_target(c)),
            &ck.output,
            &ch.chair_id,
            by_remit(),
        )
        .await;
        if ck.ok {
            return String::new();
        }
        let kr = ch.serves.as_ref().map(|s| s.key_result.as_str()).unwrap_or_default();
        format!(
            "The declared check fails, so key result {kr} is not met: `{}` in {}.\n{}\nMake it pass, then finish_chain \
             again; a check that cannot pass is stated anew with state_check, and the owner reads why.",
            c.name, c.repo, ck.output
        )
    }
}

/// What a chain's watches say of the signal its result is seen through (`Check::seen`): open, a watch on it now; watched,
/// the chain subscribed one at some point; fired, the newest watch on it fired and none was subscribed since, so the breach
/// it caught has not been answered.
#[derive(Debug, Clone, Copy, Default)]
pub struct TelemetryWatch {
    pub open: bool,
    pub watched: bool,
    pub fired: bool,
}

/// The door a check is read at: the Chair's finish, or the landing of a stage (land with a plan).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckDoor {
    AtFinish,
    AtStage,
}

/// What a chain whose key result is checked by c still lacks to pass door, one line each, empty when it passes (ADR-0068).
/// Verification is the product: a key result with no valid check passes neither door. A result that ships (ships: the
/// chain lands a canary stage, or c reads production) also needs the telemetry it is seen through, an incident plan in
/// words, and a watch on that telemetry: watched by the finish, open at a stage, and a watch that fired holds the next
/// stage until the Chair has reversed per its plan and watches again. A stage always ships. It is pure: the caller reads
/// the check, the capability and the watches.
pub fn check_gaps(c: Option<&Check>, ships: bool, w: TelemetryWatch, door: CheckDoor) -> Vec<String> {
    let Some(c) = c else {
        return vec!["no check is declared for this key result".to_owned()];
    };
    if let Err(err) = c.valid() {
        return vec![format!(
            "no check is declared for this key result ({})",
            err.to_string().replace('\n', "; ")
        )];
    }
    if !ships && c.kind != CheckKind::Reading && door != CheckDoor::AtStage {
        return Vec::new();
    }
    let mut gaps = Vec::new();
    let seen = c.seen();
    if seen.is_empty() {
        gaps.push(
            "no telemetry: the signal its result is seen through once it runs (a fact:, event: or sensor key)"
                .to_owned(),
        );
    }
    if c.incident.trim().is_empty() {
        gaps.push("no incident plan: what breaks, how it is seen, who reverses it and how".to_owned());
    }
    if seen.is_empty() {
        return gaps;
    }
    match door {
        CheckDoor::AtStage if w.fired && !w.open => gaps.push(format!(
            "the watch on {seen} fired and nothing has answered it: reverse as the incident plan says (a landing with undo, or revert_landing), \
             then subscribe a watch on {seen} again before the next stage"
        )),
        CheckDoor::AtStage if !w.open => gaps.push(format!(
            "no open watch on {seen}: subscribe one over the stage's window, its condition the breach"
        )),
        CheckDoor::AtFinish if !w.watched => gaps.push(format!(
            "the chain never watched {seen}: subscribe a watch on it over the window its result shows in"
        )),
        _ => {}
    }
    gaps
}
