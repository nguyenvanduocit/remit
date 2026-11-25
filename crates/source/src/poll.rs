//! A source is polled when Remit itself polls what changed in it, for a system that has no webhook to point at Remit
//! (ADR-0047, a sensor). The poller calls `Set::poll` with the cursor of its last read; the call passes the gate as an
//! agent's tool call does (the connection, the policy with the effect read, the deadline, the recorder), and the items it
//! answers are cleaned once more here (`clean_poll`) before the poller turns each into an event. A provider that can be
//! polled answers `Some` to `Provider::poller`; the query is its own grammar, written by the owner through the Assistant.

use std::sync::Arc;

use grant::Effect;
use jiff::Timestamp;
use serde_json::Value;

use crate::connection::{Resolved, Set, key};
use crate::error::{BoxFuture, Error, Result, err, fail};
use crate::gate::{Caller, Form, Gate};
use crate::redact::{clean_texts, redact_text, render_value, scrub};
use crate::source::{Kind, Provider};
use crate::text::shorten;

/// One thing a provider found new or changed. `id` names the thing, `version` changes whenever the thing changes in a way
/// an owner would want to hear (so id@version is the event's sender id); `body` is the item as JSON, already rendered
/// (no credential field).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollItem {
    pub id: String,
    pub version: String,
    pub body: String,
}

/// What a poll answers: items oldest first (at most limit); the cursor is opaque, pass it back on the next call.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PollResult {
    pub items: Vec<PollItem>,
    pub cursor: String,
}

/// The poll of a provider whose reads can be polled. An empty cursor means "start from now": return NO items and a cursor
/// that marks now (so a new signal never replays history).
pub trait Pollable: Send + Sync {
    fn poll<'a>(&'a self, query: &'a str, cursor: &'a str, limit: usize) -> BoxFuture<'a, Result<PollResult>>;
}

/// The name a poll goes by where a tool's would be: the Call a Recorder is told of and the gate's errors. No agent calls
/// it; Remit's poller does, as the Caller it names.
pub const POLL_CALL: &str = "poll";

/// The most of one free text (an issue's body, a monitor's message) an item carries, in bytes.
pub const POLL_TEXT_MAX: usize = 4 << 10;

impl Set {
    /// What changed in a connection of a source since `cursor`: at most `limit` items, oldest first, and the cursor to pass
    /// next time. `connection` is a name or empty for the only configured one, as for a tool. It passes the gate with the
    /// effect read and the form of a typed tool, so the Policy decides it for `caller` and the Recorder is told of it once,
    /// whatever the outcome. A returned error is safe to show.
    pub async fn poll(
        &self,
        caller: &Caller,
        source: &str,
        connection: &str,
        query: &str,
        cursor: &str,
        limit: usize,
    ) -> Result<PollResult> {
        let gate = self.poll_gate(source);
        let label_source = gate.source.clone();
        gate.pass(caller, POLL_CALL, connection, &Value::Null, move |res| async move {
            let Some(poll) = res.poll else {
                return fail!("{label_source} cannot be polled");
            };
            if limit < 1 {
                return fail!("{POLL_CALL}: limit {limit} is below 1");
            }
            let r = poll.poll(query, cursor, limit).await?;
            if r.items.len() > limit {
                return fail!(
                    "{} answered {} items to a poll of at most {limit}",
                    res.label,
                    r.items.len()
                );
            }
            clean_poll(r)
        })
        .await
    }

    /// The gate of a source that can be polled; for any other name, a gate whose resolution fails with the reason, so the
    /// call is still recorded once.
    fn poll_gate(&self, source: &str) -> Gate {
        match self.all().into_iter().find(|k| k.id == source && self.pollable(k)) {
            Some(k) => self.gate(&k),
            None => self.refusing_gate(source, self.not_pollable(source).to_string()),
        }
    }

    /// A poll of one connection: a read, made by Remit's own code with the credential in the backend (form tool).
    pub(crate) fn poll_resolved(&self, p: &Arc<dyn Provider>, k: &Kind, connection: &str) -> Result<Resolved> {
        let Some(poller) = Arc::clone(p).poller() else {
            return Err(self.not_pollable(&k.id));
        };
        let mut res = Resolved::new(key(&k.id, connection), connection, Effect::Read, Form::Tool);
        res.poll = Some(poller);
        Ok(res)
    }

    /// That `source` cannot be polled, naming the sources that can.
    fn not_pollable(&self, source: &str) -> Error {
        let mut what = format!("there is no source {source:?}");
        let mut can = Vec::new();
        for k in self.all() {
            if k.id == source {
                what = format!("{source} cannot be polled");
            }
            if self.pollable(&k) {
                can.push(k.id);
            }
        }
        if can.is_empty() {
            return err!("{what}: no source on this server can be polled");
        }
        err!("{what}: the sources that can be polled are {}", can.join(", "))
    }

    /// Whether Remit can poll the source `k`: its provider answers a poller.
    fn pollable(&self, k: &Kind) -> bool {
        self.template(k).poller().is_some()
    }
}

/// `r` as it may be stored and shown: each body one JSON value whose credential fields are dropped and whose every text has
/// token shapes removed and email addresses masked. It is the gate's net under each provider's own.
fn clean_poll(r: PollResult) -> Result<PollResult> {
    let mut items = Vec::with_capacity(r.items.len());
    for it in r.items {
        let body = clean_body(&it.body).map_err(|e| err!("item {}: {e}", redact_text(&it.id)))?;
        items.push(PollItem {
            id: redact_text(&it.id),
            version: redact_text(&it.version),
            body,
        });
    }
    Ok(PollResult {
        items,
        cursor: r.cursor,
    })
}

fn clean_body(raw: &str) -> Result<String> {
    let mut de = serde_json::Deserializer::from_str(raw);
    let v: Value = serde::Deserialize::deserialize(&mut de).map_err(|e| err!("the body is not JSON: {e}"))?;
    if de.end().is_err() {
        return fail!("the body is more than one JSON value");
    }
    render_value(&clean_texts(v))
}

/// A provider's poll answer with every value of its connection's credential removed from the items and from the error.
/// What an upstream echoes back (a key pasted into a monitor's message, an error quoting the request) has no shape
/// `redact_text` knows; the provider knows its values. An error with nothing to remove is returned as it is, so its kind
/// survives.
pub(crate) fn without_secrets(r: Result<PollResult>, secrets: &[String]) -> Result<PollResult> {
    let strip = |s: &str| scrub(s, secrets);
    match r {
        Err(e) => {
            let text = e.to_string();
            let stripped = strip(&text);
            if stripped == text {
                Err(e)
            } else {
                Err(Error::msg(stripped))
            }
        }
        Ok(mut r) => {
            for it in &mut r.items {
                *it = PollItem {
                    id: strip(&it.id),
                    version: strip(&it.version),
                    body: strip(&it.body),
                };
            }
            Ok(r)
        }
    }
}

/// Where a provider's poll stopped: the stamp of the newest thing it handed over, and the ids of the things handed over at
/// that stamp, so a thing that changes in the same instant as the last one is neither lost nor handed over twice. As a
/// cursor it reads "<RFC3339 stamp>" or "<RFC3339 stamp> <id>,<id>", each id escaped as a URL path segment so a comma or a
/// space in one cannot split it. The zero stamp (0001-01-01T00:00:00Z, which older
/// stored cursors hold) is a baseline that found nothing: whatever appears after it is new, and nothing
/// existed before it to replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollMark {
    at: Timestamp,
    ids: Vec<String>,
}

/// 0001-01-01T00:00:00Z, the zero time of a mark.
fn zero_time() -> Timestamp {
    Timestamp::from_second(-62_135_596_800).unwrap_or(Timestamp::MIN)
}

impl Default for PollMark {
    fn default() -> Self {
        Self {
            at: zero_time(),
            ids: Vec::new(),
        }
    }
}

impl PollMark {
    /// The mark a cursor says, or why it is not one.
    pub fn parse(cursor: &str) -> Result<Self> {
        let not_one = || {
            err!(
                "the cursor {:?} is not one a poll of this source returned",
                shorten(cursor, 80)
            )
        };
        let trimmed = cursor.trim();
        let (stamp, ids) = trimmed.split_once(' ').unwrap_or((trimmed, ""));
        let at: Timestamp = stamp.parse().map_err(|_| not_one())?;
        let ids = if ids.is_empty() {
            Vec::new()
        } else {
            ids.split(',')
                .map(|id| urlescape::unescape(id).ok_or_else(not_one))
                .collect::<Result<_>>()?
        };
        Ok(Self { at, ids })
    }

    /// A mark at `at` with nothing handed over there.
    #[must_use]
    pub const fn at_time(at: Timestamp) -> Self {
        Self { at, ids: Vec::new() }
    }

    /// Whether the mark is the baseline that found nothing.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.at == zero_time()
    }

    /// The stamp of the newest thing handed over.
    #[must_use]
    pub const fn at(&self) -> Timestamp {
        self.at
    }

    /// Whether the thing `id`, stamped `at`, was handed over up to this mark: it is older, or as old and among its ids.
    #[must_use]
    pub fn handed(&self, at: Timestamp, id: &str) -> bool {
        at < self.at || (at == self.at && self.ids.iter().any(|i| i == id))
    }

    /// Moves the mark over the thing `id` stamped `at`; things may come in any order.
    pub fn take(&mut self, at: Timestamp, id: &str) {
        if at > self.at {
            self.at = at;
            self.ids = vec![id.to_owned()];
        } else if at == self.at && !self.ids.iter().any(|i| i == id) {
            self.ids.push(id.to_owned());
        }
    }
}

impl std::fmt::Display for PollMark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", rfc3339_nano(self.at))?;
        if !self.ids.is_empty() {
            let ids: Vec<String> = self.ids.iter().map(|id| urlescape::path_segment(id)).collect();
            write!(f, " {}", ids.join(","))?;
        }
        Ok(())
    }
}

/// `t` as RFC 3339 in UTC: the fraction only when there is one, without trailing zeros.
#[must_use]
pub fn rfc3339_nano(t: Timestamp) -> String {
    t.to_string()
}

/// `t` as RFC 3339 in UTC: whole seconds.
#[must_use]
pub fn rfc3339(t: Timestamp) -> String {
    t.strftime("%Y-%m-%dT%H:%M:%SZ").to_string()
}
