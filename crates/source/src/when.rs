//! The times an agent names: now, a span back from now, or an RFC 3339 instant.

use jiff::{SignedDuration, Timestamp};

use crate::error::{Result, err};

/// The time `s` names: empty is now, a duration with a sign (-30m, -7d) is relative to now, anything else is RFC 3339.
pub fn parse_when(s: &str, now: Timestamp) -> Result<Timestamp> {
    let s = s.trim();
    if s.is_empty() || s == "now" {
        return Ok(now);
    }
    if let Some(rest) = s.strip_prefix('-') {
        let d = parse_duration(rest)?;
        return now.checked_sub(d).map_err(|_| err!("{s:?} is too far from now"));
    }
    if let Some(rest) = s.strip_prefix('+') {
        let d = parse_duration(rest)?;
        return now.checked_add(d).map_err(|_| err!("{s:?} is too far from now"));
    }
    s.parse::<Timestamp>()
        .map_err(|_| err!("{s:?} is neither RFC3339 (2026-10-30T02:00:00Z) nor relative (-30m, -6h, -7d)"))
}

/// A duration (`1h30m`, `90s`, `1.5h`), or a number of days with `d` (`7d`, `0.5d`).
pub(crate) fn parse_duration(s: &str) -> Result<SignedDuration> {
    let not = || err!("{s:?} is not a duration");
    if let Some(days) = s.strip_suffix('d') {
        let n: f64 = days.parse().map_err(|_| not())?;
        if !n.is_finite() {
            return Err(not());
        }
        return SignedDuration::try_from_secs_f64(n * 86_400.0).map_err(|_| not());
    }
    parse_unsigned_span(s).ok_or_else(not)
}

/// A duration with an optional sign: `-1h30m`, `90s`, `+1.5h`; units `ns`, `us`, `ms`, `s`, `m`, `h`, no days.
pub(crate) fn parse_span(s: &str) -> Option<SignedDuration> {
    match s.strip_prefix('-') {
        Some(rest) => parse_unsigned_span(rest)?.checked_neg(),
        None => parse_unsigned_span(s.strip_prefix('+').unwrap_or(s)),
    }
}

/// A duration without its sign (the caller has taken it).
fn parse_unsigned_span(s: &str) -> Option<SignedDuration> {
    if s == "0" {
        return Some(SignedDuration::ZERO);
    }
    if s.is_empty() {
        return None;
    }
    let mut total_nanos: i128 = 0;
    let mut rest = s;
    while !rest.is_empty() {
        let digits_end = rest
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(rest.len());
        let (number, after) = rest.split_at_checked(digits_end)?;
        if number.is_empty() || number == "." {
            return None;
        }
        let unit_end = after
            .find(|c: char| c.is_ascii_digit() || c == '.')
            .unwrap_or(after.len());
        let (unit, next) = after.split_at_checked(unit_end)?;
        let per: i128 = match unit {
            "ns" => 1,
            "us" | "µs" | "μs" => 1_000,
            "ms" => 1_000_000,
            "s" => 1_000_000_000,
            "m" => 60_000_000_000,
            "h" => 3_600_000_000_000,
            _ => return None,
        };
        let (whole, frac) = number.split_once('.').unwrap_or((number, ""));
        let whole: i128 = if whole.is_empty() { 0 } else { whole.parse().ok()? };
        let mut part = whole.checked_mul(per)?;
        // The fraction, digit by digit: each digit is worth a tenth of the one before.
        let mut scale = per;
        for d in frac.chars() {
            scale = scale.checked_div(10)?;
            let digit = i128::from(d.to_digit(10)?);
            part = part.checked_add(digit.checked_mul(scale)?)?;
        }
        total_nanos = total_nanos.checked_add(part)?;
        rest = next;
    }
    let nanos = i64::try_from(total_nanos).ok()?;
    Some(SignedDuration::from_nanos(nanos))
}

/// A duration as text: `25s`, `1m30s`, `2h0m0s`, `500ms`.
#[must_use]
pub fn duration_text(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    let nanos = d.subsec_nanos();
    if secs == 0 {
        return match nanos {
            0 => "0s".to_owned(),
            n if n < 1_000 => format!("{n}ns"),
            n if n < 1_000_000 => format!("{}µs", trim_fraction(n, 1_000)),
            n => format!("{}ms", trim_fraction(n, 1_000_000)),
        };
    }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    let s_text = if nanos == 0 {
        s.to_string()
    } else {
        let frac = format!("{nanos:09}");
        format!("{s}.{}", frac.trim_end_matches('0'))
    };
    if h > 0 {
        format!("{h}h{m}m{s_text}s")
    } else if m > 0 {
        format!("{m}m{s_text}s")
    } else {
        format!("{s_text}s")
    }
}

/// `n / unit` with its fraction, trailing zeros dropped.
fn trim_fraction(n: u32, unit: u32) -> String {
    let whole = n.checked_div(unit).unwrap_or(0);
    let rest = n.checked_rem(unit).unwrap_or(0);
    if rest == 0 {
        return whole.to_string();
    }
    let width = unit.to_string().len().saturating_sub(1);
    let frac = format!("{rest:0width$}");
    format!("{whole}.{}", frac.trim_end_matches('0'))
}
