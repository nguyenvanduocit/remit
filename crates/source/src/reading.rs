//! A reading is one number a source computes, as the source computes it, read once a minute at most (ADR-0046). It is how a
//! number an outside system tracks becomes a signal: a sensor whose query is a reading writes one event per minute it reads,
//! whose body carries `value` (null, with `no_data` true, when the source has no number), and a measure (ADR-0045) is a
//! condition on that field. A source's poll takes a reading as a query that starts with `reading:`, in a grammar of its own
//! after that word (Datadog: datadog_reading.rs; Grafana: grafana_reading.rs); how a reading is polled is the same for every
//! source, and is here.

use std::future::Future;

use jiff::{RoundMode, Timestamp, TimestampRound, Unit};
use serde::Serialize;

use crate::error::{Result, err};
use crate::poll::{PollItem, PollResult, rfc3339};
use crate::redact::render;
use crate::text::shorten;

/// The first word of every reading's query, whatever source or plugin reads it.
pub const READING_PREFIX: &str = "reading:";

/// The id of every reading of a signal: a signal has one query, so one thing is read and id@version is its minute.
const READING_ID: &str = "reading";

/// Whether a sensor's query asks for a reading and not for the source's other poll (Datadog's monitors, Grafana's alerts).
#[must_use]
pub fn is_reading(query: &str) -> bool {
    query.trim().starts_with(READING_PREFIX)
}

/// The poll of a reading: `read` is the body of the event of the minute `to`, holding the number as the source computes it
/// there. The cursor is the minute last read: a poll in a minute already read answers nothing, and a poll after a gap
/// answers the number as it is now, never the ones in between (a reading is a state, not a history). The first poll, with no
/// cursor, reads once to prove the credential and the query and answers no item and the minute now, so a new signal's first
/// event is the next minute's.
pub async fn reading_poll<B, F, Fut>(now: Timestamp, cursor: &str, read: F) -> Result<PollResult>
where
    B: Serialize,
    F: FnOnce(Timestamp) -> Fut,
    Fut: Future<Output = Result<B>>,
{
    let last = if cursor.is_empty() {
        None
    } else {
        Some(cursor.parse::<Timestamp>().map_err(|_| {
            err!(
                "the cursor {:?} is not one a poll of this source returned",
                shorten(cursor, 80)
            )
        })?)
    };
    let to = now
        .round(TimestampRound::new().smallest(Unit::Minute).mode(RoundMode::Trunc))
        .map_err(|e| err!("{e}"))?;
    if let Some(last) = last
        && to <= last
    {
        return Ok(PollResult {
            items: Vec::new(),
            cursor: cursor.to_owned(),
        });
    }
    let body = read(to).await?;
    let stamp = rfc3339(to);
    if last.is_none() {
        return Ok(PollResult {
            items: Vec::new(),
            cursor: stamp,
        });
    }
    let raw = render(&body)?;
    Ok(PollResult {
        items: vec![PollItem {
            id: READING_ID.to_owned(),
            version: stamp.clone(),
            body: raw,
        }],
        cursor: stamp,
    })
}
