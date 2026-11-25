//! A subscription's own time (docs/adr/0064): pure, like the rest of this crate. A schedule acts at its subscription's
//! `due_at`; what it fires is that instant, so a firing recorded again (a restart, a retry) is the same firing.

use model::{Schedule, Time};

use crate::cron::{CronError, cron_next};

/// The first instant schedule `s` acts at, counted at `now`: its one instant, or the next of its cron after `now`. One
/// that does not repeat and names no instant never acts.
pub fn schedule_first(s: &Schedule, now: Time) -> Result<Time, CronError> {
    if s.once() {
        s.at.ok_or_else(|| CronError::Never { spec: s.cron.clone() })
    } else {
        cron_next(&s.cron, now)
    }
}

/// The instant schedule `s` acts at after an instant it acted at, or skipped, at `now`. A repeating one is counted again
/// from `now`, so the instants missed while the server was down are one and not a run of them. `None` when it is over: one
/// that does not repeat, and one whose cron cannot be counted, since the alternative is to act at every tick.
pub fn schedule_next(s: &Schedule, now: Time) -> Option<Time> {
    if s.once() {
        return None;
    }
    cron_next(&s.cron, now).ok()
}
