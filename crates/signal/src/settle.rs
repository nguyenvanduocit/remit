//! What becomes of a firing whose processor failed (docs/adr/0064, record, then deliver): it is tried again after a wait
//! that doubles, a few times, and then it fails for good and holds its subscription in place, where its owner's agent
//! retries it or skips it. Nothing is dead-lettered: a held subscription keeps its cursor and every firing after the held
//! one waits behind it, so a fix resumes it and loses nothing.

use std::fmt;

use model::{Duration, Firing, FiringCause, FiringStatus, HoldReason, Subscription, Time};

use crate::evaluate::{Effect, Hold, apply};
use crate::instant;

/// How many times a firing is given to its processor before it fails for good. Five attempts over about seven minutes
/// ride out what passes on its own (a service restarting, a network blip, a plugin starting cold) and still show the owner
/// a held subscription within minutes when it does not pass.
pub const DELIVERY_ATTEMPTS: i64 = 5;
/// The wait after the first failed attempt; each wait after doubles it, up to `LAST_BACKOFF`.
const FIRST_BACKOFF: Duration = Duration(30 * Duration::SECOND.0);
const LAST_BACKOFF: Duration = Duration::minutes(15);

/// Why an attempt of a firing failed: a failure trying again may mend, or a permanent one and the hold it puts its
/// subscription on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryError {
    message: String,
    permanent: Option<HoldReason>,
}

impl DeliveryError {
    /// A failure trying again may mend.
    pub fn new(err: impl fmt::Display) -> Self {
        Self {
            message: err.to_string(),
            permanent: None,
        }
    }
}

/// `err` marked as a failure trying again does not mend (no processor does the action, the action is refused): `settle`
/// fails the firing at its first attempt and holds its subscription for `reason`.
pub fn permanent(reason: HoldReason, err: impl fmt::Display) -> DeliveryError {
    DeliveryError {
        message: err.to_string(),
        permanent: Some(reason),
    }
}

impl fmt::Display for DeliveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DeliveryError {}

/// How long a firing waits after its attempt-th failed attempt before it is tried again.
fn backoff(attempt: i64) -> Duration {
    let mut wait = FIRST_BACKOFF;
    for _ in 1..attempt {
        if wait.0 >= LAST_BACKOFF.0.checked_div(2).unwrap_or(0) {
            return LAST_BACKOFF;
        }
        wait = Duration(wait.0.saturating_mul(2));
    }
    wait
}

/// What an attempt of firing `f` of `sub` that failed with `err` at `now` makes of them both. Short of its last attempt,
/// `f` is pending and tried again after its backoff, and `sub` does not move. At its last attempt, or at its first when
/// `err` is permanent, `f` has failed and `sub` is held on it, which the hold effect says.
pub fn settle(
    mut sub: Subscription,
    mut f: Firing,
    err: &DeliveryError,
    now: Time,
) -> (Subscription, Firing, Vec<Effect>) {
    f.attempts = f.attempts.saturating_add(1);
    f.last_error = err.message.clone();
    if err.permanent.is_none() && f.attempts < DELIVERY_ATTEMPTS {
        f.status = FiringStatus::Pending;
        f.next_attempt = Some(instant::add(now, backoff(f.attempts)));
        return (sub, f, Vec::new());
    }
    let reason = err.permanent.unwrap_or(HoldReason::FiringFailed);
    f.status = FiringStatus::Failed;
    f.next_attempt = None;
    sub.hold = Some(model::Hold {
        reason,
        detail: f.last_error.clone(),
        firing: f.id(),
        since: now,
    });
    sub.tally.failed = sub.tally.failed.saturating_add(1);
    let hold = Effect::Hold(Hold {
        reason,
        detail: f.last_error.clone(),
    });
    (sub, f, vec![hold])
}

/// Lifts the hold failed firing `f` puts `sub` on by trying `f` again: it is pending from its first attempt, due at once.
pub fn retry(mut sub: Subscription, mut f: Firing) -> (Subscription, Firing) {
    sub.hold = None;
    f.status = FiringStatus::Pending;
    f.attempts = 0;
    f.next_attempt = None;
    (sub, f)
}

/// Does again firing `f` that a skip gave up, as when the owner overturns the skip: it is pending from its first attempt,
/// due at once, and goes before its subscription's later firings. Its subscription does not move: its readings were
/// judged when `f` was recorded.
pub fn redo(mut f: Firing) -> Firing {
    f.status = FiringStatus::Pending;
    f.attempts = 0;
    f.next_attempt = None;
    f.last_error = String::new();
    f
}

/// Lifts the hold failed firing `f` puts `sub` on by giving `f` up: `f` is skipped, the readings it covers are judged, and
/// the subscription goes on with the readings after them. One that was to end with `f` (its deadline's firing, or the
/// firing of its one instant) ends.
pub fn skip_firing(sub: Subscription, mut f: Firing) -> (Subscription, Firing) {
    f.status = FiringStatus::Skipped;
    f.next_attempt = None;
    let mut moves = Vec::new();
    if let Some(top) = f.seqs.iter().max() {
        moves.push(Effect::Advance(*top));
    }
    if f.cause == FiringCause::Expired || sub.schedule.as_ref().is_some_and(model::Schedule::once) {
        moves.push(Effect::End);
    }
    let mut sub = apply(sub, &moves);
    sub.hold = None;
    (sub, f)
}
