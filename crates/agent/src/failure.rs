use std::sync::LazyLock;

use jiff::civil::{Date, Time};
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan, Zoned};
use regex::Regex;
use wire::runtime::FailureCategory;

/// `at` as a time the runtime contract carries; none past the years a time holds, which no provider says.
pub(crate) fn time_of(at: Timestamp) -> Option<model::Time> {
    time::OffsetDateTime::from_unix_timestamp_nanos(at.as_nanosecond())
        .ok()
        .map(model::Time)
}

/// The category of an assistant error kind (SDKAssistantMessageError) that is the provider's fault. The rest
/// (invalid_request, model_not_found, max_output_tokens, unknown) are not: waiting for the provider would not change the
/// answer.
pub(crate) fn claude_failure(kind: &str) -> Option<FailureCategory> {
    match kind {
        "rate_limit" | "billing_error" | "account_on_hold" => Some(FailureCategory::Limit),
        "overloaded" | "server_error" => Some(FailureCategory::Service),
        "authentication_failed" | "oauth_org_not_allowed" | "verification_required" | "cloud_credential_error" => {
            Some(FailureCategory::Access)
        }
        _ => None,
    }
}

/// How Claude Code says when a limit resets, at the end of what it said, read when no rate_limit_event said it: "resets
/// 9:40am (UTC)", "resets Nov 6 at 10pm (Asia/Saigon)".
static RESET_WORDS: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"resets (?:([A-Z][a-z]{2}) (\d{1,2}),? (?:at )?)?(\d{1,2})(?::(\d{2}))?(am|pm) \(([^)]+)\)").ok()
});

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// When a limit resets, read from what the provider said at now; `None` when it says no time it can be read as. A time
/// with no day is its next one after now; a day with no year is the one nearest after now, so a reset said in December
/// for January falls in the next year.
pub(crate) fn resets_at(text: &str, now: Timestamp) -> Option<Timestamp> {
    let m = RESET_WORDS.as_ref()?.captures(text)?;
    let tz = TimeZone::get(m.get(6)?.as_str()).ok()?;
    let hour12: i8 = m.get(3)?.as_str().parse().ok()?;
    let minute: i8 = m.get(4).map_or(Some(0), |g| g.as_str().parse().ok())?;
    if !(1..=12).contains(&hour12) {
        return None;
    }
    let hour = hour12
        .checked_rem(12)?
        .checked_add(if m.get(5)?.as_str() == "pm" { 12 } else { 0 })?;
    let clock = Time::new(hour, minute, 0, 0).ok()?;
    let local = now.to_zoned(tz.clone());
    let Some(month) = m.get(1) else {
        let mut at = local.date().to_datetime(clock).to_zoned(tz).ok()?;
        if at.timestamp() <= now {
            at = at.checked_add(1.day()).ok()?;
        }
        return Some(at.timestamp());
    };
    let month = i8::try_from(MONTHS.iter().position(|name| *name == month.as_str())?.checked_add(1)?).ok()?;
    let day: i8 = m.get(2)?.as_str().parse().ok()?;
    let on = |year: i16| -> Option<Zoned> {
        Date::new(year, month, day)
            .ok()?
            .to_datetime(clock)
            .to_zoned(tz.clone())
            .ok()
    };
    let at = on(local.year())?;
    if at.timestamp() < now.checked_sub(24.hours()).ok()? {
        return on(local.year().checked_add(1)?).map(|z| z.timestamp());
    }
    Some(at.timestamp())
}
