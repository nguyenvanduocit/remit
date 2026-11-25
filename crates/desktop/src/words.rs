//! Times as the owner reads them, the web's way (`web/src/ui.ts`): a length of time (27s, 4m, 1h 12m, 2d 5h) and a time
//! of day in his own clock.

use link::Millis;
use time::{OffsetDateTime, UtcOffset};

/// A length of time: 27s, 4m, 1h 12m, 2d 5h.
pub fn span(ms: Millis) -> String {
    let s = ms.saturating_add(500) / 1000;
    let m = s / 60;
    match m {
        0 => format!("{s}s"),
        1..60 => format!("{m}m"),
        60..1440 => format!("{}h {}m", m / 60, m % 60),
        _ => format!("{}d {}h", m / 1440, (m % 1440) / 60),
    }
}

/// Milliseconds since the Unix epoch of `t`; none before it.
pub fn at(t: OffsetDateTime) -> Option<Millis> {
    u64::try_from(t.unix_timestamp_nanos() / 1_000_000).ok()
}

/// How long ago `at` was, at `now`.
pub fn span_since(at: OffsetDateTime, now: Millis) -> String {
    span(now.saturating_sub(self::at(at).unwrap_or(0)))
}

/// The time of day of `ms` in the owner's clock (`offset`): 14:05.
pub fn clock(ms: Millis, offset: UtcOffset) -> String {
    let Ok(t) = OffsetDateTime::from_unix_timestamp_nanos(i128::from(ms).saturating_mul(1_000_000)) else {
        return String::new();
    };
    let t = t.to_offset(offset);
    format!("{:02}:{:02}", t.hour(), t.minute())
}
