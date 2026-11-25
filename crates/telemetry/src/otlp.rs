//! OTLP/HTTP JSON (`ExportLogsServiceRequest`, `ExportMetricsServiceRequest`, `ExportTraceServiceRequest`) read into wide
//! events. Pure: bytes in, records out. A 64-bit integer may come as a JSON string, as the OTLP JSON mapping writes it, or as
//! a number, as Claude Code writes it; both read. What a record does not carry reads as empty.

use serde::Deserialize;
use serde_json::{Map, Number, Value};

use crate::{Kind, Record};

/// Why a request's body was not read.
#[derive(Debug, thiserror::Error)]
#[error("not an OTLP JSON {what} request: {source}")]
pub struct DecodeError {
    what: &'static str,
    source: serde_json::Error,
}

/// The attributes every record of an account repeats, which name the person signed in: the card already names its
/// account, so they are not kept.
const IDENTITY: [&str; 5] = [
    "user.id",
    "user.email",
    "user.account_uuid",
    "user.account_id",
    "organization.id",
];

/// The records of a logs request: one per log record, named by its body (Claude Code's event name).
pub fn logs(body: &[u8]) -> Result<Vec<Record>, DecodeError> {
    let req: LogsRequest = parse("logs", body)?;
    let mut out = Vec::new();
    for rl in req.resource_logs {
        let resource = attributes(&rl.resource.attributes);
        for sl in rl.scope_logs {
            let scope = scope_attributes(&sl.scope);
            for r in sl.log_records {
                let mut attrs = merged(&resource, &scope, &r.attributes);
                if !r.severity_text.is_empty() {
                    attrs.insert("otel.severity".into(), Value::String(r.severity_text));
                }
                let name = match r.body.value() {
                    Value::String(s) if !s.is_empty() => s,
                    _ => r.event_name,
                };
                out.push(Record {
                    at_ns: r.time_unix_nano.or(r.observed_time_unix_nano).unwrap_or_default(),
                    kind: Kind::Event,
                    name,
                    value: None,
                    trace_id: r.trace_id,
                    span_id: r.span_id,
                    parent_id: String::new(),
                    attrs,
                });
            }
        }
    }
    Ok(out)
}

/// The records of a metrics request: one per data point, its value the point's number (a histogram's sum, its count among
/// the attributes).
pub fn metrics(body: &[u8]) -> Result<Vec<Record>, DecodeError> {
    let req: MetricsRequest = parse("metrics", body)?;
    let mut out = Vec::new();
    for rm in req.resource_metrics {
        let resource = attributes(&rm.resource.attributes);
        for sm in rm.scope_metrics {
            let scope = scope_attributes(&sm.scope);
            for m in sm.metrics {
                let (data, shape) = m.data();
                let Some(data) = data else { continue };
                for p in &data.data_points {
                    let mut attrs = merged(&resource, &scope, &p.attributes);
                    attrs.insert("otel.metric_type".into(), Value::String(shape.into()));
                    if !m.unit.is_empty() {
                        attrs.insert("otel.unit".into(), Value::String(m.unit.clone()));
                    }
                    if let Some(t) = temporality(data.aggregation_temporality) {
                        attrs.insert("otel.temporality".into(), Value::String(t.into()));
                    }
                    if let Some(count) = p.count {
                        attrs.insert("otel.count".into(), Value::from(count));
                    }
                    out.push(Record {
                        at_ns: p.time_unix_nano.unwrap_or_default(),
                        kind: Kind::Metric,
                        name: m.name.clone(),
                        value: p.as_double.or(p.as_int.and_then(int_to_f64)).or(p.sum),
                        trace_id: String::new(),
                        span_id: String::new(),
                        parent_id: String::new(),
                        attrs,
                    });
                }
            }
        }
    }
    Ok(out)
}

/// The records of a traces request: one per span, at its start, its value its duration in milliseconds.
pub fn traces(body: &[u8]) -> Result<Vec<Record>, DecodeError> {
    let req: TracesRequest = parse("traces", body)?;
    let mut out = Vec::new();
    for rs in req.resource_spans {
        let resource = attributes(&rs.resource.attributes);
        for ss in rs.scope_spans {
            let scope = scope_attributes(&ss.scope);
            for s in ss.spans {
                let mut attrs = merged(&resource, &scope, &s.attributes);
                if let Some(code) = s.status.code {
                    attrs.insert("otel.status_code".into(), Value::from(code));
                }
                if !s.status.message.is_empty() {
                    attrs.insert("otel.status_message".into(), Value::String(s.status.message));
                }
                if !s.events.is_empty() {
                    let events = s
                        .events
                        .iter()
                        .map(|e| {
                            let mut o = attributes(&e.attributes);
                            o.insert("name".into(), Value::String(e.name.clone()));
                            o.insert("at_ns".into(), Value::from(e.time_unix_nano.unwrap_or_default()));
                            Value::Object(o)
                        })
                        .collect();
                    attrs.insert("otel.events".into(), Value::Array(events));
                }
                let start = s.start_time_unix_nano.unwrap_or_default();
                let millis = s
                    .end_time_unix_nano
                    .and_then(|end| end.checked_sub(start))
                    .and_then(|ns| int_to_f64(ns / 1_000_000));
                out.push(Record {
                    at_ns: start,
                    kind: Kind::Span,
                    name: s.name,
                    value: millis,
                    trace_id: s.trace_id,
                    span_id: s.span_id,
                    parent_id: s.parent_span_id,
                    attrs,
                });
            }
        }
    }
    Ok(out)
}

fn parse<'a, T: Deserialize<'a>>(what: &'static str, body: &'a [u8]) -> Result<T, DecodeError> {
    serde_json::from_slice(body).map_err(|source| DecodeError { what, source })
}

/// OTLP's `AggregationTemporality`.
fn temporality(t: i64) -> Option<&'static str> {
    match t {
        1 => Some("delta"),
        2 => Some("cumulative"),
        _ => None,
    }
}

fn int_to_f64(n: i64) -> Option<f64> {
    Number::from(n).as_f64()
}

fn attributes(kvs: &[KeyValue]) -> Map<String, Value> {
    kvs.iter()
        .filter(|kv| !IDENTITY.contains(&kv.key.as_str()))
        .map(|kv| (kv.key.clone(), kv.value.value()))
        .collect()
}

fn scope_attributes(scope: &Scope) -> Map<String, Value> {
    let mut m = attributes(&scope.attributes);
    if !scope.name.is_empty() {
        m.insert("otel.scope.name".into(), Value::String(scope.name.clone()));
    }
    m
}

/// A record's attributes over its scope's over its resource's.
fn merged(resource: &Map<String, Value>, scope: &Map<String, Value>, own: &[KeyValue]) -> Map<String, Value> {
    let mut m = resource.clone();
    m.extend(scope.iter().map(|(k, v)| (k.clone(), v.clone())));
    m.extend(attributes(own));
    m
}

/// A 64-bit integer as OTLP JSON writes it (a string) or as a number.
fn int64<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Int {
        Number(i64),
        Text(String),
    }
    Ok(match Option::<Int>::deserialize(d)? {
        Some(Int::Number(n)) => Some(n),
        Some(Int::Text(s)) => s.parse().ok(),
        None => None,
    })
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct LogsRequest {
    resource_logs: Vec<ResourceLogs>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ResourceLogs {
    resource: Resource,
    scope_logs: Vec<ScopeLogs>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ScopeLogs {
    scope: Scope,
    log_records: Vec<LogRecord>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct LogRecord {
    #[serde(deserialize_with = "int64")]
    time_unix_nano: Option<i64>,
    #[serde(deserialize_with = "int64")]
    observed_time_unix_nano: Option<i64>,
    severity_text: String,
    body: AnyValue,
    event_name: String,
    attributes: Vec<KeyValue>,
    trace_id: String,
    span_id: String,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct MetricsRequest {
    resource_metrics: Vec<ResourceMetrics>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ResourceMetrics {
    resource: Resource,
    scope_metrics: Vec<ScopeMetrics>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ScopeMetrics {
    scope: Scope,
    metrics: Vec<Metric>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Metric {
    name: String,
    unit: String,
    sum: Option<MetricData>,
    gauge: Option<MetricData>,
    histogram: Option<MetricData>,
}

impl Metric {
    /// Its data points and what kind of metric holds them.
    fn data(&self) -> (Option<&MetricData>, &'static str) {
        if let Some(d) = &self.sum {
            (Some(d), "sum")
        } else if let Some(d) = &self.gauge {
            (Some(d), "gauge")
        } else {
            (self.histogram.as_ref(), "histogram")
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct MetricData {
    data_points: Vec<DataPoint>,
    aggregation_temporality: i64,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct DataPoint {
    attributes: Vec<KeyValue>,
    #[serde(deserialize_with = "int64")]
    time_unix_nano: Option<i64>,
    as_double: Option<f64>,
    #[serde(deserialize_with = "int64")]
    as_int: Option<i64>,
    /// A histogram's.
    sum: Option<f64>,
    #[serde(deserialize_with = "int64")]
    count: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct TracesRequest {
    resource_spans: Vec<ResourceSpans>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ResourceSpans {
    resource: Resource,
    scope_spans: Vec<ScopeSpans>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ScopeSpans {
    scope: Scope,
    spans: Vec<Span>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Span {
    trace_id: String,
    span_id: String,
    parent_span_id: String,
    name: String,
    #[serde(deserialize_with = "int64")]
    start_time_unix_nano: Option<i64>,
    #[serde(deserialize_with = "int64")]
    end_time_unix_nano: Option<i64>,
    attributes: Vec<KeyValue>,
    events: Vec<SpanEvent>,
    status: Status,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SpanEvent {
    name: String,
    #[serde(deserialize_with = "int64")]
    time_unix_nano: Option<i64>,
    attributes: Vec<KeyValue>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Status {
    code: Option<i64>,
    message: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Resource {
    attributes: Vec<KeyValue>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Scope {
    name: String,
    attributes: Vec<KeyValue>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct KeyValue {
    key: String,
    value: AnyValue,
}

/// OTLP's `AnyValue`: one of its fields is set.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AnyValue {
    string_value: Option<String>,
    bool_value: Option<bool>,
    #[serde(deserialize_with = "int64")]
    int_value: Option<i64>,
    double_value: Option<f64>,
    array_value: Option<ArrayValue>,
    kvlist_value: Option<KvList>,
    bytes_value: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ArrayValue {
    values: Vec<AnyValue>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct KvList {
    values: Vec<KeyValue>,
}

impl AnyValue {
    fn value(&self) -> Value {
        if let Some(s) = &self.string_value {
            Value::String(s.clone())
        } else if let Some(b) = self.bool_value {
            Value::Bool(b)
        } else if let Some(n) = self.int_value {
            Value::from(n)
        } else if let Some(f) = self.double_value {
            Number::from_f64(f).map_or(Value::Null, Value::Number)
        } else if let Some(a) = &self.array_value {
            Value::Array(a.values.iter().map(AnyValue::value).collect())
        } else if let Some(kv) = &self.kvlist_value {
            Value::Object(kv.values.iter().map(|kv| (kv.key.clone(), kv.value.value())).collect())
        } else if let Some(b) = &self.bytes_value {
            Value::String(b.clone())
        } else {
            Value::Null
        }
    }
}
