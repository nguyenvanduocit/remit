//! The server's clock: the system's time in the server's offset, the local zone's. The offset is read once, in `main`
//! before any thread starts (the `time` crate reads it only then), and handed on.

use std::sync::Arc;

use time::{OffsetDateTime, UtcOffset};

/// This machine's offset from UTC, read before any thread starts; UTC when it cannot be read.
pub fn local_offset() -> UtcOffset {
    UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC)
}

/// The clock every port that reads the time is given.
pub fn model_clock(offset: UtcOffset) -> Arc<dyn Fn() -> model::Time + Send + Sync> {
    Arc::new(move || model::Time(OffsetDateTime::now_utc().to_offset(offset)))
}
