//! The Signals table: the newest readings and firings whose key matches a pattern, newest first, each in the owner's
//! words as the board words them (signalboard.rs).

use crate::Pipeline;
use crate::signalboard::{firing_lines, namer_of, reading_line};
use chains::Result;
use wire::app::SignalRow;

/// The most rows one read gives.
const SIGNAL_ROWS_MOST: i64 = 500;

impl Pipeline {
    /// The newest limit readings and firings whose key matches pattern (every key when empty), newest first.
    pub async fn signal_rows(&self, pattern: &str, limit: i64) -> Result<Vec<SignalRow>> {
        let pattern = if pattern.is_empty() {
            "*".to_owned()
        } else {
            pattern.to_owned()
        };
        let limit = limit.clamp(1, SIGNAL_ROWS_MOST);
        let (chains, subs, signals, rows) = self
            .store
            .read(move |r| {
                Ok::<_, store::Error>((
                    r.chains()?,
                    r.subscriptions()?,
                    r.event_signals()?,
                    r.signal_rows(&pattern, limit)?,
                ))
            })
            .await?;
        let n = namer_of(&chains, &subs, &signals);
        Ok(rows
            .into_iter()
            .filter_map(|row| match (row.reading, row.firing) {
                (Some(r), _) => Some(SignalRow {
                    key: r.key.0.clone(),
                    reading: Some(reading_line(&r)),
                    firing: None,
                }),
                (None, Some(f)) => Some(SignalRow {
                    key: f.key.0.clone(),
                    reading: None,
                    firing: firing_lines(std::slice::from_ref(&f), &n).pop(),
                }),
                (None, None) => None,
            })
            .collect())
    }
}
