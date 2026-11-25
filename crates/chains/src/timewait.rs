//! A wait for time (docs/adr/0064) is a subscription that keeps a schedule: its time is its own (`Subscription::schedule`
//! and `due_at`), made, changed and ended with it, and the Tick fires it when its instant comes (`signal::evaluate`).
//! `wait_for` is the one function that stores one, for a routine, a shipped job, a goal check's deadline, a help's
//! deadline and a Chair's check-back alike.

use model::{Schedule, Subscription, Time};
use store::Tx;

use crate::{Error, Result};

/// sub due at the first instant of its schedule after now: a routine or shipped job counts its cadence from when it is
/// made or changed, never from an instant before. sub keeps a schedule.
pub fn counted_from(mut sub: Subscription, now: Time) -> Result<Subscription> {
    let s = sub
        .schedule
        .as_ref()
        .ok_or_else(|| Error::refused(format!("subscription {} keeps no schedule", sub.id)))?;
    sub.due_at = Some(signal::schedule_first(s, now).map_err(|e| Error::refused(e.to_string()))?);
    Ok(sub)
}

/// sub keeping a schedule that acts once, at `at`, due then.
pub fn once_at(mut sub: Subscription, at: Time) -> Subscription {
    sub.schedule = Some(Schedule {
        at: Some(at),
        ..Schedule::default()
    });
    sub.due_at = Some(at);
    sub
}

/// Stores sub, a subscription that keeps a schedule, in tx, and returns it as stored. A wait set again answers the
/// instants of the schedule it is set with, not what came before: a firing of an earlier instant still to be delivered is
/// cancelled.
pub fn wait_for(tx: &Tx<'_>, sub: Subscription) -> store::Result<Subscription> {
    tx.cancel_pending_firings(&sub.id)?;
    tx.put_subscription(sub)
}
