//! Packing the events that have settled, on the automation's schedule.

use model::Time;

use crate::Pipeline;

/// How often the automation has the store pack the events that have settled (`store::Store::pack_events`).
pub(crate) const PACK_EVERY: time::Duration = time::Duration::DAY;

/// How long an event goes unchanged before it is packed: its turn, and the owner's look at it while it was news, are long
/// over by then.
const EVENTS_SETTLE: time::Duration = time::Duration::days(7);

impl Pipeline {
    /// Has the store pack the events unchanged for `EVENTS_SETTLE`, and says in the log how many it packed. It runs in the
    /// background, a short write transaction at a time, so no change waits for it long; dropping the future stops it, and
    /// what it packed stays packed.
    pub(crate) async fn pack_events(&self, now: Time) {
        let before = now.0.checked_sub(EVENTS_SETTLE).map_or_else(Time::default, Time);
        match self.store.pack_events(before).await {
            Err(err) => tracing::error!(%err, "pack settled events"),
            Ok(n) if n > 0 => tracing::info!(events = n, "packed settled events"),
            Ok(_) => {}
        }
    }
}
