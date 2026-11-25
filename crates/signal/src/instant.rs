//! Instant arithmetic on model's `Time` and `Duration`, which the evaluator counts with: overflow saturates, as
//! no instant Remit stores comes near the ends of what `time` holds.

use model::{Duration, Time};

/// `t` plus `d`; `t` itself past the range `time` holds.
pub(crate) fn add(t: Time, d: Duration) -> Time {
    t.0.checked_add(time::Duration::nanoseconds(d.0)).map_or(t, Time)
}

/// How long after `b` `a` is (negative when before), saturating at what a Duration holds.
pub(crate) fn since(a: Time, b: Time) -> Duration {
    let nanos = a.0.unix_timestamp_nanos().saturating_sub(b.0.unix_timestamp_nanos());
    Duration(i64::try_from(nanos).unwrap_or(if nanos < 0 { i64::MIN } else { i64::MAX }))
}

/// `t` in milliseconds since the Unix epoch.
pub(crate) fn unix_milli(t: Time) -> i64 {
    let millis = t.0.unix_timestamp_nanos().div_euclid(1_000_000);
    i64::try_from(millis).unwrap_or(if millis < 0 { i64::MIN } else { i64::MAX })
}

/// The earlier of two instants, either of which may be none.
pub(crate) fn earliest(a: Option<Time>, b: Option<Time>) -> Option<Time> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// `t` as RFC 3339 with whole seconds, and its offset (Z for UTC).
pub(crate) fn rfc3339(t: Time) -> String {
    let at = t.0;
    let (h, m, _) = at.offset().as_hms();
    let offset = if at.offset().is_utc() {
        "Z".to_owned()
    } else {
        let sign = if h < 0 || m < 0 { '-' } else { '+' };
        format!("{sign}{:02}:{:02}", h.unsigned_abs(), m.unsigned_abs())
    };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{offset}",
        at.year(),
        u8::from(at.month()),
        at.day(),
        at.hour(),
        at.minute(),
        at.second()
    )
}

/// `d` rounded to the second, halves away from zero.
pub(crate) fn round_second(d: Duration) -> Duration {
    let s = Duration::SECOND.0;
    let rem = d.0.checked_rem(s).unwrap_or(0);
    let down = d.0.saturating_sub(rem);
    if rem.unsigned_abs().saturating_mul(2) >= s.unsigned_abs() {
        Duration(if d.0 < 0 {
            down.saturating_sub(s)
        } else {
            down.saturating_add(s)
        })
    } else {
        Duration(down)
    }
}

/// `d` written as a duration: `1h2m3.5s`, `4m0s`, `1.5s`, `0s`, and below a second `ms`, `µs` or `ns`.
pub(crate) fn span_text(d: Duration) -> String {
    if d.0 == 0 {
        return "0s".to_owned();
    }
    let sign = if d.0 < 0 { "-" } else { "" };
    let n = d.0.unsigned_abs();
    if n < 1_000_000_000 {
        let (unit, div) = if n < 1_000 {
            ("ns", 1)
        } else if n < 1_000_000 {
            ("µs", 1_000)
        } else {
            ("ms", 1_000_000)
        };
        return format!("{sign}{}{unit}", fraction(n, div));
    }
    let secs = n.checked_div(1_000_000_000).unwrap_or(0);
    let (h, m) = (
        secs.checked_div(3600).unwrap_or(0),
        secs.checked_rem(3600).unwrap_or(0).checked_div(60).unwrap_or(0),
    );
    let s = fraction(n.checked_rem(60_000_000_000).unwrap_or(0), 1_000_000_000);
    match (h, m) {
        (0, 0) => format!("{sign}{s}s"),
        (0, _) => format!("{sign}{m}m{s}s"),
        _ => format!("{sign}{h}h{m}m{s}s"),
    }
}

/// `n / div` written with the fraction it has, trailing zeros trimmed.
fn fraction(n: u64, div: u64) -> String {
    let whole = n.checked_div(div).unwrap_or(0);
    let rest = n.checked_rem(div).unwrap_or(0);
    if rest == 0 {
        return whole.to_string();
    }
    let digits = div.to_string().len().saturating_sub(1);
    let frac = format!("{rest:0digits$}");
    format!("{whole}.{}", frac.trim_end_matches('0'))
}
