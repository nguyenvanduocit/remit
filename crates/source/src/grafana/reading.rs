//! A Grafana reading (reading.rs, ADR-0046) is one number a PromQL expression answers, evaluated by a Prometheus-compatible
//! datasource of Grafana at the minute it is read: an error rate, a latency percentile, a queue's depth. A sensor's query
//! asks for one as `reading:promql [datasource=<uid or name>] <PromQL expression>`; the window and the aggregation are the
//! expression's own (`rate(x[5m])`, `sum(...)`, `histogram_quantile(0.95, ...)`), and an empty datasource is Grafana's only
//! prometheus one. Grafana answers it through the datasource proxy (GET /api/datasources/proxy/uid/<uid>/api/v1/query), as
//! grafana_promql does, so a reading is the number the owner's own panel shows.

use std::fmt;

use serde::Serialize;
use serde_json::Value;

use super::{Grafana, prom_response};
use crate::error::{Result, err, fail};
use crate::poll::{PollResult, rfc3339};
use crate::reading::{READING_PREFIX, reading_poll};
use crate::text::shorten;

const LANGUAGE: &str = "promql";
const GRAMMAR: &str = "`reading:promql [datasource=<uid or name>] <PromQL expression>`";
const DATASOURCE_KEY: &str = "datasource=";

/// A reading as a sensor's query asks Grafana for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GrafanaReading {
    pub datasource: String,
    pub query: String,
}

/// The first word of `s` and the text after it, both without the space around them.
fn cut_word(s: &str) -> (&str, &str) {
    let s = s.trim();
    match s.find(char::is_whitespace) {
        Some(i) => (s.get(..i).unwrap_or_default(), s.get(i..).unwrap_or_default().trim()),
        None => (s, ""),
    }
}

/// The reading a sensor's query asks for, or why it is not one.
pub(super) fn parse_reading(query: &str) -> Result<GrafanaReading> {
    let (head, mut rest) = cut_word(query);
    let language = head.strip_prefix(READING_PREFIX).unwrap_or(head);
    if language != LANGUAGE {
        return fail!(
            "a Grafana reading reads promql, not {:?}: its query is {GRAMMAR}",
            shorten(language, 40)
        );
    }
    let mut datasource = String::new();
    let (word, tail) = cut_word(rest);
    if let Some(ds) = word.strip_prefix(DATASOURCE_KEY) {
        datasource = ds.to_owned();
        rest = tail;
    }
    if rest.is_empty() {
        return fail!(
            "a Grafana reading needs its PromQL expression, such as sum(rate(http_requests_total{{status=~\"5..\"}}[5m])): its query is {GRAMMAR}"
        );
    }
    Ok(GrafanaReading {
        datasource,
        query: rest.to_owned(),
    })
}

/// The query of a sensor that reads it: what an agent copies into a sensor.
impl fmt::Display for GrafanaReading {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{READING_PREFIX}{LANGUAGE} {DATASOURCE_KEY}{} {}",
            self.datasource, self.query
        )
    }
}

/// What an event of a reading carries: the number a measure reads, and what it is the number of.
#[derive(Serialize)]
struct ReadingBody {
    /// The uid of the datasource that evaluated it.
    datasource: String,
    query: String,
    /// The minute it was evaluated at.
    at: String,
    /// None when the expression has no value there.
    value: Option<f64>,
    no_data: bool,
}

impl Grafana {
    /// Reads the number the query asks for, at the minute it is read (`reading_poll`).
    pub(super) async fn poll_reading(&self, query: &str, cursor: &str) -> Result<PollResult> {
        let r = parse_reading(query)?;
        reading_poll(self.now.now(), cursor, |to| async move {
            let uid = self.prometheus(&r.datasource).await?;
            let raw = self.instant(&uid, &r.query, to).await?;
            let value = prom_number(&raw)?;
            Ok(ReadingBody {
                datasource: uid,
                query: r.query,
                at: rfc3339(to),
                value,
                no_data: value.is_none(),
            })
        })
        .await
    }
}

/// The one number an instant query answers, None when it has none: no series, or NaN, which PromQL answers for a ratio of
/// nothing (0/0). An answer of more than one series, or of a range, is more than one number, and a reading is one, so it is
/// refused with what to change.
pub(super) fn prom_number(raw: &[u8]) -> Result<Option<f64>> {
    let (r, data) = prom_response(raw)?;
    if r.status != "success" {
        return fail!("prometheus {}: {}", r.error_type, shorten(&r.error, 300));
    }
    let result = data.result.unwrap_or_default();
    let sample = match data.result_type.as_str() {
        "vector" => match result.as_slice() {
            [] => return Ok(None),
            [one] => {
                let value = one.get("value").and_then(Value::as_array).map(Vec::as_slice);
                let Some([_, v]) = value else {
                    return fail!("grafana: the series is not a Prometheus sample");
                };
                let Some(text) = v.as_str() else {
                    return fail!("grafana: the sample's value is not a number");
                };
                return prom_float(text);
            }
            many => {
                return fail!(
                    "grafana: the query answers {} series and a reading is one number: aggregate them, as sum(...) or max(...), or match one series",
                    many.len()
                );
            }
        },
        "scalar" => match result.as_slice() {
            [_, v] => v.clone(),
            _ => return fail!("grafana: the scalar is not a Prometheus sample"),
        },
        other => {
            return fail!(
                "grafana: the query answers a {other} and a reading is one number: drop the range selector at its top, or wrap it, as in rate(x[5m])"
            );
        }
    };
    let Some(text) = sample.as_str() else {
        return Err(err!("grafana: the scalar's value is not a number"));
    };
    prom_float(text)
}

/// The number of a Prometheus sample's value, None for NaN.
fn prom_float(text: &str) -> Result<Option<f64>> {
    match text.parse::<f64>() {
        Err(_) => fail!("grafana: the value {:?} is not a number", shorten(text, 40)),
        Ok(v) if v.is_nan() => Ok(None),
        Ok(v) if v.is_infinite() => {
            fail!("grafana: the value is {text}, not a number a measure can read: the expression divides by zero")
        }
        Ok(v) => Ok(Some(v)),
    }
}
