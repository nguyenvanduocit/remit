//! A Datadog reading (reading.rs, ADR-0046) is one number Datadog computes over a window: the errors an app logged in the
//! last hour, the p95 of its latency, the sessions its users opened today. A sensor's query asks for one as
//! `reading:<source> [window=<duration>] [agg=<aggregator>] <search>`: the source is metrics, logs or rum; the window is from
//! 1m to 31d, default 1h; the aggregator is how a metric's points become one number (avg, min, max, sum, last; default avg),
//! and the count of a log or RUM search is its only one. Datadog computes it (POST /api/v2/query/scalar, an endpoint that
//! reads), so a reading is the same number the owner sees in Datadog's own notebook.

use std::fmt;

use jiff::{RoundMode, SignedDuration, Timestamp, TimestampRound, Unit};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{Datadog, decode};
use crate::error::{Error, Result, err, fail};
use crate::poll::{PollResult, rfc3339};
use crate::reading::{READING_PREFIX, reading_poll};
use crate::text::{float_prec, shorten};
use crate::when::{duration_text, parse_duration};

const DEFAULT_WINDOW: SignedDuration = SignedDuration::from_hours(1);
const MIN_WINDOW: SignedDuration = SignedDuration::from_mins(1);
const MAX_WINDOW: SignedDuration = SignedDuration::from_hours(31 * 24);

/// What a reading can read, each with the aggregators that turn what it finds into one number, the default first. Event
/// sources (logs, rum) count the events their search matches.
const SOURCES: [(&str, &[&str]); 3] = [
    ("metrics", &["avg", "min", "max", "sum", "last"]),
    ("logs", &["count"]),
    ("rum", &["count"]),
];

/// A reading as an owner or an agent asks for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Reading {
    pub source: String,
    pub aggregator: String,
    pub query: String,
    pub window: SignedDuration,
}

impl Reading {
    /// The reading of `source`, `window`, `aggregator` and `query`, each empty one at its default, or why it is not one.
    pub(crate) fn new(source: &str, window: &str, aggregator: &str, query: &str) -> Result<Self> {
        let Some((_, aggregators)) = SOURCES.iter().find(|(name, _)| *name == source) else {
            return fail!("source {:?} is not metrics, logs or rum", shorten(source, 40));
        };
        let mut r = Self {
            source: source.to_owned(),
            aggregator: aggregators.first().copied().unwrap_or_default().to_owned(),
            query: query.trim().to_owned(),
            window: DEFAULT_WINDOW,
        };
        if !window.is_empty() {
            match parse_duration(window) {
                Ok(d) if (MIN_WINDOW..=MAX_WINDOW).contains(&d) => r.window = d,
                _ => {
                    return fail!(
                        "window {:?} is not a duration from 1m to 31d such as 15m, 6h or 7d",
                        shorten(window, 40)
                    );
                }
            }
        }
        if !aggregator.is_empty() {
            if !aggregators.contains(&aggregator) {
                return fail!(
                    "agg {:?} is not one {source} reads: {}",
                    shorten(aggregator, 40),
                    or_list(aggregators)
                );
            }
            aggregator.clone_into(&mut r.aggregator);
        }
        if r.query.is_empty() {
            if source == "metrics" {
                return fail!("a metrics reading needs its metric query, such as avg:system.cpu.user{{service:api}}");
            }
            "*".clone_into(&mut r.query);
        }
        Ok(r)
    }

    /// The reading a sensor's query asks for: `reading:<source>` then any of `window=` and `agg=`, then the search.
    pub(crate) fn parse(query: &str) -> Result<Self> {
        let (head, mut rest) = cut_word(query);
        let Some(source) = head.strip_prefix(READING_PREFIX) else {
            return fail!("a reading's query is `reading:<metrics|logs|rum> [window=1h] [agg=avg] <search>`");
        };
        let (mut window, mut aggregator) = ("", "");
        loop {
            let (word, tail) = cut_word(rest);
            match word.split_once('=') {
                Some(("window", v)) => window = v,
                Some(("agg", v)) => aggregator = v,
                _ => break,
            }
            rest = tail;
        }
        Self::new(source, window, aggregator, rest)
            .map_err(|e| err!("reading query: {e} (`reading:<metrics|logs|rum> [window=1h] [agg=avg] <search>`)"))
    }

    fn request(&self, from: Timestamp, to: Timestamp) -> Value {
        let mut q = json!({"data_source": self.source, "name": "a"});
        if let Some(m) = q.as_object_mut() {
            if self.source == "metrics" {
                m.insert("query".to_owned(), json!(self.query));
                m.insert("aggregator".to_owned(), json!(self.aggregator));
            } else {
                m.insert("compute".to_owned(), json!({"aggregation": self.aggregator}));
                m.insert("search".to_owned(), json!({"query": self.query}));
                m.insert("indexes".to_owned(), json!(["*"]));
            }
        }
        json!({"data": {"type": "scalar_request", "attributes": {
            "formulas": [{"formula": "a"}],
            "queries": [q],
            "from": from.as_millisecond(),
            "to": to.as_millisecond(),
        }}})
    }
}

/// The query of a sensor that reads it: what an agent copies into a sensor.
impl fmt::Display for Reading {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{READING_PREFIX}{} window={} agg={}",
            self.source,
            window_text(self.window),
            self.aggregator
        )?;
        if self.query != "*" {
            write!(f, " {}", self.query)?;
        }
        Ok(())
    }
}

/// `words` as "a, b or c".
fn or_list(words: &[&str]) -> String {
    match words.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} or {last}", rest.join(", ")),
        _ => words.concat(),
    }
}

/// The first word of `s` and the text after it, both without the space around them.
fn cut_word(s: &str) -> (&str, &str) {
    let s = s.trim();
    match s.find(char::is_whitespace) {
        Some(i) => {
            let (word, rest) = s.split_at(i);
            (word, rest.trim())
        }
        None => (s, ""),
    }
}

/// `d` as `parse_duration` reads it back: 7d, 6h, 15m.
pub(crate) fn window_text(d: SignedDuration) -> String {
    let secs = d.as_secs();
    if d.subsec_nanos() == 0 && secs > 0 {
        for (unit, suffix) in [(86_400, 'd'), (3_600, 'h'), (60, 'm')] {
            if secs.checked_rem(unit) == Some(0) {
                return format!("{}{suffix}", secs.checked_div(unit).unwrap_or_default());
            }
        }
    }
    duration_text(d.unsigned_abs())
}

// ---- the call ----

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ScalarResponse {
    data: ScalarData,
    errors: Option<Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ScalarData {
    attributes: ScalarAttributes,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ScalarAttributes {
    columns: Option<Vec<Column>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Column {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    values: Option<Vec<Value>>,
}

impl ScalarResponse {
    /// The one number the answer holds, none when Datadog has no data for it. An answer grouped by a tag or a facet is more
    /// than one number, and a reading is one, so it is refused with the group it found.
    fn number(&self) -> Result<Option<f64>> {
        if let Some(e) = &self.errors {
            let text = match e {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            let text = text.trim_matches('"');
            if !text.is_empty() && text != "null" {
                return fail!("datadog: {}", shorten(text, 300));
            }
        }
        let mut values: Option<&[Value]> = None;
        for c in self.data.attributes.columns.as_deref().unwrap_or_default() {
            match c.kind.as_str() {
                "group" => {
                    return fail!(
                        "datadog: the query is grouped by {} and a reading is one number: drop the `by {{...}}` or the group_by so the query answers one value",
                        c.name
                    );
                }
                "number" if values.is_none() => values = Some(c.values.as_deref().unwrap_or_default()),
                _ => {}
            }
        }
        let Some(values) = values else {
            return fail!("datadog: the answer has no number: check the query");
        };
        match values {
            [] => Ok(None),
            [Value::Null] => Ok(None),
            [v] => v
                .as_f64()
                .map(Some)
                .ok_or_else(|| Error::msg(format!("datadog: the number is not a number: {v}"))),
            more => fail!(
                "datadog: the query answers {} numbers and a reading is one: aggregate it",
                more.len()
            ),
        }
    }
}

impl Datadog {
    /// `r` over from..to as Datadog computes it, none when it has no data there.
    async fn scalar(&self, r: &Reading, from: Timestamp, to: Timestamp) -> Result<Option<f64>> {
        let api = self.api()?;
        let raw = api.post_json("/api/v2/query/scalar", &r.request(from, to)).await?;
        let res: ScalarResponse = decode(&raw)?;
        res.number()
    }

    /// The one number a reading has now, with the query that makes a signal of it.
    pub(super) async fn reading(&self, input: ReadingInput) -> Result<String> {
        let r = Reading::new(&input.source, &input.window, &input.agg, &input.query)?;
        let to = self
            .now
            .now()
            .round(TimestampRound::new().smallest(Unit::Minute).mode(RoundMode::Trunc))
            .map_err(|e| err!("{e}"))?;
        let from = to.checked_sub(r.window).map_err(|e| err!("{e}"))?;
        let v = self.scalar(&r, from, to).await?;
        let what = format!(
            "{} {} of {} over {} ending {}",
            r.source,
            r.aggregator,
            r.query,
            window_text(r.window),
            rfc3339(to)
        );
        Ok(match v {
            None => format!(
                "{what}: no data. Nothing reports it in that window: check the query, the tags and that the app sends it.\nAs a signal: {r}"
            ),
            Some(v) => format!("{what}: {}\nAs a signal: {r}", float_prec(v, 21)),
        })
    }

    /// Reads the number the query asks for, over the window that ends at the minute it is read (`reading_poll`).
    pub(super) async fn poll_reading(&self, query: &str, cursor: &str) -> Result<PollResult> {
        let r = Reading::parse(query)?;
        reading_poll(self.now.now(), cursor, |to| async move {
            let from = to.checked_sub(r.window).map_err(|e| err!("{e}"))?;
            let v = self.scalar(&r, from, to).await?;
            Ok(ReadingBody {
                source: r.source.clone(),
                query: r.query.clone(),
                aggregator: r.aggregator.clone(),
                window: window_text(r.window),
                from: rfc3339(from),
                to: rfc3339(to),
                value: v,
                no_data: v.is_none(),
            })
        })
        .await
    }
}

// ---- the tool: try a reading before it is a signal ----

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct ReadingInput {
    /// what the reading reads: metrics, logs or rum
    source: String,
    /// for metrics, the metric query such as avg:app.error_rate{service:api} or sum:trace.http.request.errors{env:prod}.as_count(), with no by-clause; for logs and rum, the search as datadog_logs takes it (empty: every event)
    #[serde(default)]
    query: String,
    /// how far back it reads, ending at the last full minute: 1m to 31d such as 15m, 6h or 7d; default 1h
    #[serde(default)]
    window: String,
    /// how the points of a metric become one number: avg (default), min, max, sum or last; logs and rum count their events
    #[serde(default)]
    agg: String,
}

/// What an event of a reading carries: the number a measure reads, and what it is the number of.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ReadingBody {
    pub source: String,
    pub query: String,
    pub aggregator: String,
    pub window: String,
    pub from: String,
    pub to: String,
    /// Null when Datadog has no data in the window.
    pub value: Option<f64>,
    pub no_data: bool,
}
