//! When a cron spec acts next, as robfig/cron's `ParseStandard` counts it: five fields, a descriptor (`@daily`), or
//! `@every <duration>`, in the server's time zone.

use std::fmt;
use std::str::FromStr;

use chrono::{Local, TimeZone};
use croner::Cron;
use model::{Duration, Time};

use crate::instant;

/// Why a cron spec gives no next instant; its text says the forms a spec takes, for the owner's Assistant to relay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CronError {
    Invalid { spec: String, message: String },
    Never { spec: String },
}

impl fmt::Display for CronError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CronError::Invalid { spec, message } => write!(
                f,
                "cron {spec:?}: {message}; five fields (minute hour day-of-month month day-of-week) or @every 6h"
            ),
            CronError::Never { spec } => write!(f, "cron {spec:?} never happens"),
        }
    }
}

impl std::error::Error for CronError {}

/// The first time after `t` that `spec` says, in the server's time zone.
pub fn cron_next(spec: &str, t: Time) -> Result<Time, CronError> {
    let invalid = |message: String| CronError::Invalid {
        spec: spec.to_owned(),
        message,
    };
    let trimmed = spec.trim();
    if let Some(every) = trimmed.strip_prefix("@every ") {
        let delay = parse_span(every.trim()).ok_or_else(|| invalid(format!("time: invalid duration {every:?}")))?;
        // robfig: a delay is whole seconds, at least one; the next instant is the whole second `delay` after t's.
        let second = Duration::SECOND.0;
        let whole = delay
            .0
            .saturating_sub(delay.0.checked_rem(second).unwrap_or(0))
            .max(second);
        let into_second = i64::from(t.0.nanosecond());
        return Ok(instant::add(t, Duration(whole.saturating_sub(into_second))));
    }
    // croner reads every robfig descriptor but @midnight, which robfig takes as @daily.
    let spec_text = if trimmed.eq_ignore_ascii_case("@midnight") {
        "@daily"
    } else {
        trimmed
    };
    let cron = Cron::from_str(spec_text).map_err(|e| invalid(e.to_string()))?;
    let from = chrono::DateTime::from_timestamp(t.0.unix_timestamp(), t.0.nanosecond())
        .ok_or_else(|| CronError::Never { spec: spec.to_owned() })?;
    let next = cron
        .find_next_occurrence(&Local.from_utc_datetime(&from.naive_utc()), false)
        .map_err(|_| CronError::Never { spec: spec.to_owned() })?;
    let at = time::OffsetDateTime::from_unix_timestamp(next.timestamp())
        .map_err(|_| CronError::Never { spec: spec.to_owned() })?;
    Ok(Time(at))
}

/// A duration written as `6h`, `1h30m`, `1.5h`, `90s`, `250ms`, signed, or `0`.
pub(crate) fn parse_span(text: &str) -> Option<Duration> {
    let (negative, mut rest) = match text.as_bytes().first() {
        Some(b'-') => (true, text.get(1..)?),
        Some(b'+') => (false, text.get(1..)?),
        _ => (false, text),
    };
    if rest == "0" {
        return Some(Duration::ZERO);
    }
    if rest.is_empty() {
        return None;
    }
    let mut total: i64 = 0;
    while !rest.is_empty() {
        let digits = rest
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(rest.len());
        let (number, after) = rest.split_at(digits);
        let unit_len = after
            .find(|c: char| c.is_ascii_digit() || c == '.')
            .unwrap_or(after.len());
        let (unit, next) = after.split_at(unit_len);
        let scale: i64 = match unit {
            "ns" => 1,
            "us" | "µs" | "μs" => 1_000,
            "ms" => 1_000_000,
            "s" => 1_000_000_000,
            "m" => 60_000_000_000,
            "h" => 3_600_000_000_000,
            _ => return None,
        };
        let (whole, frac) = number.split_once('.').unwrap_or((number, ""));
        if whole.is_empty() && frac.is_empty() {
            return None;
        }
        let whole: i64 = if whole.is_empty() { 0 } else { whole.parse().ok()? };
        let mut part = whole.checked_mul(scale)?;
        let mut place = scale;
        for d in frac.chars() {
            place = place.checked_div(10)?;
            part = part.checked_add(i64::from(d.to_digit(10)?).checked_mul(place)?)?;
        }
        total = total.checked_add(part)?;
        rest = next;
    }
    Some(Duration(if negative { total.checked_neg()? } else { total }))
}
