//! The signal log forgets while the server is up (docs/adr/0064, Start position and retention): the shipped subscription
//! sub_run_forget runs the job forget every `FORGET_EVERY`. A reading goes only when no one needs it (the store's floor: no
//! live subscription still has to judge it, no firing still to be done or held on covers it, and it is not the newest of
//! its key) and it is past its kind's history: a stream's readings after `STREAM_WINDOW`, a state's beyond its newest
//! `STATE_READINGS_KEPT`. A firing done, skipped or cancelled goes after `STREAM_WINDOW`; one pending or failed never does.
//! A held or paused subscription so keeps every reading it has not judged, without limit.

use model::Time;

use crate::Pipeline;

/// How often the job forget runs.
pub(crate) const FORGET_EVERY: time::Duration = time::Duration::minutes(15);
/// How long a stream's readings, events and facts, are kept: far past the time a writer retries.
pub(crate) const STREAM_WINDOW: time::Duration = time::Duration::days(7);
/// How many readings of one state are kept, whatever their age.
pub(crate) const STATE_READINGS_KEPT: i64 = 1000;

impl Pipeline {
    /// Drops what the signal log keeps past its history at now: the readings, then the firings on them, which say only
    /// what was done. A pass that fails is logged; the next one does its work.
    pub(crate) async fn forget_signals(&self, now: Time) {
        let since = now.0.checked_sub(STREAM_WINDOW).map_or_else(Time::default, Time);
        let forgot = self
            .store
            .update(move |tx| -> store::Result<i64> {
                let readings = tx.forget_readings(since, STATE_READINGS_KEPT)?;
                tx.forget_firings_before(since)?;
                Ok(readings)
            })
            .await;
        match forgot {
            Err(err) => tracing::error!(%err, "the signal log was not trimmed"),
            Ok(readings) if readings > 0 => tracing::info!(readings, "the signal log forgot what no one needs"),
            Ok(_) => {}
        }
    }
}
