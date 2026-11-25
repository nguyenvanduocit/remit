//! Taking the store's scheduled snapshot when one is due.

use model::Time;

use crate::Pipeline;

/// How often the automation asks the store whether a scheduled snapshot is due. The store decides from the snapshots on
/// disk, so a restart neither takes one early nor forgets one; this only keeps the asking off every Tick.
pub(crate) const SNAPSHOT_CHECK_EVERY: time::Duration = time::Duration::HOUR;

impl Pipeline {
    /// Has the store copy the database when its last scheduled snapshot is `Config::snapshot_every` old, and says in the log
    /// what it did. It runs in the background: the copy takes as long as the database is big, and the scheduler does not
    /// wait for it. Dropping the future (a shutdown that runs out of time) stops the copy and leaves none.
    pub(crate) async fn snapshot_if_due(&self, now: Time) {
        match self.store.snapshot_if_due(now, self.cfg.snapshot_every).await {
            Err(err) => tracing::error!(%err, "database snapshot"),
            Ok(Some(file)) => tracing::info!(file = %file.display(), "database snapshot"),
            Ok(None) => {}
        }
    }
}
