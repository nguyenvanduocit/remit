//! Datadog is what the apps did while they ran: their logs, the spans of their traces, their metrics, the monitors that watch
//! them and the events (a deploy among them) that mark when something changed. An agent reads it to check a change against
//! the running app, whether the error it fixed has stopped and whether a new one began; Remit polls its monitors to hear one
//! that leaves OK, or reads one number it computes (`poll`, datadog/reading.rs). Every tool reads: they call GET, except the
//! log count and the reading, which call the endpoints that only aggregate. Credentials: DATADOG_API_KEY and DATADOG_APP_KEY,
//! the application key limited to the read scopes (logs_read_data, apm_read, timeseries_query, monitors_read, events_read);
//! DATADOG_SITE is optional and is the site of the organisation (datadoghq.eu, us5.datadoghq.com, ...).

use std::collections::BTreeMap;
use std::sync::{Arc, LazyLock};

use jiff::{SignedDuration, Timestamp};
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::api::{Api, Http, query};
use crate::cloudwatch::one_line;
use crate::env::{Clock, Env, missing};
use crate::error::{BoxFuture, Result, err, fail};
use crate::grafana::{labels_of, range_summary};
use crate::limit::Limit;
use crate::poll::{POLL_TEXT_MAX, PollItem, PollMark, PollResult, Pollable, rfc3339, rfc3339_nano, without_secrets};
use crate::reading::is_reading;
use crate::redact::{mask_emails, redact_text, render, render_value};
use crate::script::InjectSpec;
use crate::source::{Provider, Shared, Tool, Var, new_tool};
use crate::text::{cut_bytes, shorten, value_text_of};
use crate::when::{duration_text, parse_duration, parse_when};

mod reading;

const DEFAULT_SITE: &str = "datadoghq.com";
/// How many logs, spans or events one call lists by default, and at most: a page an agent reads whole.
const PAGE: Limit = Limit::new(25, 100);
/// How much of one entry's attributes an agent reads.
const ATTR_CUT: usize = 500;
/// The most groups one count lists.
const COUNT_GROUPS: i64 = 50;
/// How many series of one metrics answer are shown.
const MAX_SERIES: usize = 40;

#[expect(clippy::expect_used, reason = "a constant pattern")]
/// The shape of a Datadog site: host labels and nothing that moves the address (a path, a port, a login).
static SITE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+$").expect("site pattern")
});

pub(crate) fn new(env: Env, shared: Shared) -> Arc<dyn Provider> {
    Arc::new(Datadog::of(env, shared))
}

/// One connection of Datadog.
pub(crate) struct Datadog {
    env: Env,
    now: Clock,
    http: Http,
    api_key: String,
    app_key: String,
    site: String,
    /// Empty when the site is not a Datadog site. A test points it at its own server.
    base: String,
}

impl Datadog {
    pub(crate) fn of(env: Env, shared: Shared) -> Self {
        let named = env.get("DATADOG_SITE");
        let (site, base) = match site(&named) {
            Some(s) => {
                let base = format!("https://api.{s}");
                (s, base)
            }
            None => (named.trim().to_lowercase(), String::new()),
        };
        Self {
            api_key: env.get("DATADOG_API_KEY"),
            app_key: env.get("DATADOG_APP_KEY"),
            env,
            now: shared.now,
            http: shared.http,
            site,
            base,
        }
    }

    fn credential_headers(&self) -> BTreeMap<String, String> {
        BTreeMap::from([
            ("DD-API-KEY".to_owned(), self.api_key.clone()),
            ("DD-APPLICATION-KEY".to_owned(), self.app_key.clone()),
        ])
    }

    fn api(&self) -> Result<Api> {
        if self.base.is_empty() {
            return fail!(
                "DATADOG_SITE {:?} is not a Datadog site such as datadoghq.com, datadoghq.eu or us5.datadoghq.com",
                self.site
            );
        }
        Ok(Api {
            name: "Datadog".to_owned(),
            base: self.base.clone(),
            headers: self.credential_headers().into_iter().collect(),
            http: self.http.clone(),
        })
    }

    /// from..to, `default_from` standing for an empty from.
    fn window(&self, from: &str, to: &str, default_from: &str) -> Result<(Timestamp, Timestamp)> {
        let now = self.now.now();
        let from = if from.trim().is_empty() { default_from } else { from };
        let start = parse_when(from, now).map_err(|e| e.context("from"))?;
        let end = parse_when(to, now).map_err(|e| e.context("to"))?;
        if end <= start {
            return fail!("to is not after from");
        }
        Ok((start, end))
    }
}

/// The Datadog site a connection's DATADOG_SITE names, datadoghq.com when it names none; none when it is not a site's name
/// (host labels only, nothing that moves the address: a path, a port, a login). Remit's exporter reads the same (ADR-0119).
#[must_use]
pub fn site(named: &str) -> Option<String> {
    let named = named.trim().to_lowercase();
    let named = if named.is_empty() {
        DEFAULT_SITE.to_owned()
    } else {
        named
    };
    SITE.is_match(&named).then_some(named)
}

/// Wraps an async method of Datadog as a tool's run.
macro_rules! tool {
    ($me:expr, $name:literal, $what:literal, $method:ident, $in:ty) => {{
        let me = Arc::clone(&$me);
        new_tool($name, $what, move |input: $in| {
            let me = Arc::clone(&me);
            async move { me.$method(input).await }
        })
    }};
}

impl Provider for Datadog {
    fn name(&self) -> String {
        "Datadog".to_owned()
    }

    fn missing(&self) -> Vec<String> {
        missing(&self.env, &["DATADOG_API_KEY", "DATADOG_APP_KEY"])
    }

    fn vars(&self) -> Vec<Var> {
        vec![
            Var::plain("DATADOG_API_KEY", "a Datadog API key of the organisation").secret(),
            Var::plain(
                "DATADOG_APP_KEY",
                "an application key that may only read (scopes logs_read_data, apm_read, timeseries_query, monitors_read, events_read)",
            )
            .secret(),
            Var::plain(
                "DATADOG_SITE",
                "the Datadog site of the organisation, only when it is not datadoghq.com (datadoghq.eu, us5.datadoghq.com, ap1.datadoghq.com)",
            )
            .optional()
            .address(),
        ]
    }

    fn tools(self: Arc<Self>) -> Vec<Tool> {
        vec![
            tool!(
                self,
                "datadog_logs",
                "Search the logs of the apps, newest first by default, one line each: time, status, service, host, message (a stack trace joined on one line, emails masked) and the log's own attributes. `query` is Datadog's log search syntax, for example `service:api status:error`, `@http.status_code:500`, `@usr.id:123` or `\"connection refused\"`; empty reads every log of the window. The default window is the last hour. A full page ends with a `cursor`: pass it back, with the same other inputs, for the next page. To count rather than read, use datadog_log_counts.",
                logs,
                ChainSearch
            ),
            tool!(
                self,
                "datadog_log_counts",
                "Count logs that match a search, over a window: one total, the total per value of `group_by` (a facet such as service, status or @http.status_code, largest first), or, with `interval` such as 5m or 1h, per time bucket. Answers 'how many errors', 'which service is loudest' and 'did the errors stop after the deploy' without reading the lines. The default window is the last hour.",
                log_counts,
                CountInput
            ),
            tool!(
                self,
                "datadog_spans",
                "Search the spans of the apps' APM traces, newest first by default, one line each: start, service, resource, duration, env, trace and span id, and the span's own attributes (the error, the HTTP status, the user). `query` is Datadog's span search syntax, for example `service:api @http.status_code:500`, `status:error resource_name:\"POST /orders\"` or `trace_id:<id>` to read one trace whole. The default window is the last hour. A full page ends with a `cursor`.",
                spans,
                ChainSearch
            ),
            tool!(
                self,
                "datadog_metrics",
                "Query metrics with Datadog's metric query syntax, such as `avg:system.cpu.user{service:api}`, `sum:trace.http.request.errors{env:prod}.as_count()` or `p95:trace.http.request.duration{service:api}`. Each series is summarised (points, first, last, min, max, average), not dumped. The default window is the last hour; Datadog picks the resolution from it, a longer window gives coarser points.",
                metrics,
                MetricsInput
            ),
            tool!(
                self,
                "datadog_monitors",
                "Search the monitors that watch the apps, with the state each is in now (OK, Alert, Warn, No Data), its type, query, tags and when it last triggered. `query` is Datadog's monitor search syntax: free words match the name, and facets narrow it, for example `status:alert`, `tag:service:api` or `type:metric`; empty lists them all. A full page continues with the next `page`. A sensor can read the monitors this query matches: each one that goes into Alert, Warn or No Data becomes an event, and one that recovers to OK does not.",
                monitors,
                MonitorsInput
            ),
            tool!(
                self,
                "datadog_reading",
                "Read one number Datadog computes over a window: a metric's average, sum, max, min or last value, or how many logs or RUM events a search matches (what an app reports of its errors, latency, sessions or plays). The window ends at the last full minute; default 1h. Use it to see what a number is now before it becomes a signal: the answer ends with the query a sensor takes, and a measure of an objective reads that signal's `value`. A metric query that groups (by {tag}) is refused: a reading is one number. No data means nothing reports it in that window.",
                reading,
                reading::ReadingInput
            ),
            tool!(
                self,
                "datadog_events",
                "Search the events of the organisation, newest first by default: the deploys, the monitor alerts and the other changes teams and tools post, one line each with time, status, service, title and tags. Use it to place a deploy on the timeline that the logs, spans and metrics show. `query` is Datadog's event search syntax, for example `sources:jenkins` or `tags:service:api`; empty reads all events of the window. The default window is the last day. A full page ends with a `cursor`.",
                events,
                Search
            ),
        ]
    }

    /// How a script reaches Datadog (form proxy): the same address and keys as the typed tools, GET anywhere, and POST only
    /// at the endpoints that search and aggregate, which take their query in a body and read.
    fn inject(&self) -> Option<InjectSpec> {
        Some(InjectSpec {
            base: self.base.clone(),
            headers: self.credential_headers(),
            read_posts: [
                "/api/v2/logs/events/search",
                "/api/v2/logs/analytics/aggregate",
                "/api/v2/spans/events/search",
                "/api/v2/spans/analytics/aggregate",
                "/api/v2/query/timeseries",
                "/api/v2/query/scalar",
            ]
            .map(str::to_owned)
            .to_vec(),
            sign: None,
            secrets: vec![self.api_key.clone(), self.app_key.clone()],
        })
    }

    fn poller(self: Arc<Self>) -> Option<Arc<dyn Pollable>> {
        Some(self)
    }
}

// ---- the three searches that list entries: logs, spans, events ----

/// What the tools that list logs, spans or events take.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct Search {
    /// the search, in Datadog's syntax for what the tool reads (the tool's description gives examples); empty is everything in the window
    #[serde(default)]
    query: String,
    /// where the window begins: RFC3339 or relative to now such as -30m, -6h, -7d; default -1h (-1d for events)
    #[serde(default)]
    from: String,
    /// where the window ends: RFC3339 or relative; default now
    #[serde(default)]
    to: String,
    /// read the oldest first instead of the newest first
    #[serde(default)]
    oldest: bool,
    /// how many to list, 1 to 100; default 25
    #[serde(default)]
    limit: i64,
    /// the cursor a full page ended with, to read the next page of the same search
    #[serde(default)]
    cursor: String,
}

/// What datadog_logs and datadog_spans take: a search, narrowed to one Remit chain when `chain` says so. Events carry no
/// chain, so datadog_events takes a `Search` and the input cannot name one.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct ChainSearch {
    /// only what one Remit chain did (ch_…, as the chain's screen shows it): every log and span Remit sends about a chain (the server's, its agents' and its children's) carries it as `remit.chain_id`; a release carries many chains, so it has a marker span for each chain it carried, which this finds, and its trace leads to the release's own work
    #[serde(default)]
    chain: String,
    #[serde(flatten)]
    search: Search,
}

/// One log, span or event of a list answer.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Entry {
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    attributes: serde_json::Map<String, Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct List {
    data: Option<Vec<Entry>>,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    meta: ListMeta,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ListMeta {
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    page: ListPage,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ListPage {
    after: Option<String>,
}

/// query narrowed to what Remit chain `chain` did: every record Remit sends carries its chain as `remit.chain_id`
/// (ADR-0119), so one search finds a chain's logs and spans in every process. An id is letters, digits, `_` and `-`:
/// anything else would change the search, not narrow it.
fn of_chain(query: &str, chain: &str) -> Result<String> {
    let chain = chain.trim();
    if chain.is_empty() {
        return Ok(query.to_owned());
    }
    if !chain.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return fail!("chain {chain:?} is not a chain id such as ch_abc123");
    }
    let scope = format!("@{}:{chain}", telemetry::attr::CHAIN_ID);
    // the agent's search is grouped, so an OR inside it cannot widen the search past the chain
    Ok(if query.trim().is_empty() {
        scope
    } else {
        format!("{scope} ({query})")
    })
}

/// The decoded answer of a Datadog endpoint.
fn decode<T: serde::de::DeserializeOwned>(raw: &[u8]) -> Result<T> {
    serde_json::from_slice(raw).map_err(|e| err!("datadog: the answer is not JSON: {e}"))
}

impl Datadog {
    /// One page of `path` (GET /api/v2/logs/events, spans/events or events).
    async fn list(&self, path: &str, input: &Search, query_of: &str, limit: usize, default_from: &str) -> Result<List> {
        let api = self.api()?;
        let (from, to) = self.window(&input.from, &input.to, default_from)?;
        let mut q = query(&[
            ("filter[from]", &rfc3339(from)),
            ("filter[to]", &rfc3339(to)),
            ("sort", if input.oldest { "timestamp" } else { "-timestamp" }),
            ("page[limit]", &limit.to_string()),
        ]);
        if !query_of.trim().is_empty() {
            q.push(("filter[query]".to_owned(), query_of.to_owned()));
        }
        if !input.cursor.is_empty() {
            q.push(("page[cursor]".to_owned(), input.cursor.clone()));
        }
        decode(&api.get(path, &q).await?)
    }

    async fn logs(&self, input: ChainSearch) -> Result<String> {
        let limit = PAGE.of("limit", input.search.limit)?;
        let query = of_chain(&input.search.query, &input.chain)?;
        let l = self
            .list("/api/v2/logs/events", &input.search, &query, limit, "-1h")
            .await?;
        Ok(layout(&l, "logs", limit, |a| {
            format!(
                "{}  {}  {}  {}  {}{}",
                str_of(a.get("timestamp")),
                str_of(a.get("status")),
                str_of(a.get("service")),
                str_of(a.get("host")),
                one_line(str_of(a.get("message"))),
                attrs_of(a.get("attributes"))
            )
        }))
    }

    async fn spans(&self, input: ChainSearch) -> Result<String> {
        let limit = PAGE.of("limit", input.search.limit)?;
        let query = of_chain(&input.search.query, &input.chain)?;
        let l = self
            .list("/api/v2/spans/events", &input.search, &query, limit, "-1h")
            .await?;
        Ok(layout(&l, "spans", limit, |a| {
            let start = str_of(a.get("start_timestamp"));
            format!(
                "{start}  {}  {}  {}  env={} trace={} span={}{}",
                str_of(a.get("service")),
                one_line(str_of(a.get("resource_name"))),
                span_duration(start, str_of(a.get("end_timestamp"))),
                str_of(a.get("env")),
                str_of(a.get("trace_id")),
                str_of(a.get("span_id")),
                attrs_of(a.get("custom"))
            )
        }))
    }

    async fn events(&self, input: Search) -> Result<String> {
        let limit = PAGE.of("limit", input.limit)?;
        let l = self.list("/api/v2/events", &input, &input.query, limit, "-1d").await?;
        Ok(layout(&l, "events", limit, |outer| {
            let inner = outer.get("attributes").and_then(Value::as_object);
            let field = |k: &str| str_of(inner.and_then(|m| m.get(k)));
            let tags = match outer.get("tags").and_then(Value::as_array) {
                Some(t) if !t.is_empty() => {
                    let words: Vec<String> = t.iter().map(value_text_of).collect();
                    format!(" tags={}", shorten(&format!("[{}]", words.join(" ")), ATTR_CUT))
                }
                _ => String::new(),
            };
            let title = if field("title").is_empty() {
                str_of(outer.get("message"))
            } else {
                field("title")
            };
            format!(
                "{}  {}  {}  {}{tags}",
                str_of(outer.get("timestamp")),
                field("status"),
                field("service"),
                one_line(title)
            )
        }))
    }
}

/// The entries as lines under a head that says what was searched, and the cursor of the next page when the page is full.
fn layout(l: &List, head: &str, limit: usize, line: impl Fn(&serde_json::Map<String, Value>) -> String) -> String {
    let data = l.data.as_deref().unwrap_or_default();
    let mut b = format!("{head}: {} shown\n", data.len());
    for e in data {
        b.push_str(&line(&e.attributes));
        b.push('\n');
    }
    if data.is_empty() {
        b.push_str("[nothing matches: widen the window or loosen the query]\n");
    }
    let full = data.len() >= limit;
    if let Some(after) = l.meta.page.after.as_deref().filter(|a| !a.is_empty())
        && full
    {
        b.push_str(&format!("[more: cursor {after}]\n"));
    }
    b.trim_end_matches('\n').to_owned()
}

/// A field that is text, or nothing.
fn str_of(v: Option<&Value>) -> &str {
    v.and_then(Value::as_str).unwrap_or_default()
}

/// The attributes of an entry as an agent reads them: credential fields removed, emails masked, cut.
fn attrs_of(v: Option<&Value>) -> String {
    let Some(v) = v.filter(|v| v.as_object().is_some_and(|m| !m.is_empty())) else {
        return String::new();
    };
    match render_value(v) {
        Ok(out) => format!(" {}", mask_emails(&shorten(&out, ATTR_CUT))),
        Err(_) => String::new(),
    }
}

/// How long a span ran, from its two timestamps; empty when either is not a time.
fn span_duration(start: &str, end: &str) -> String {
    let (Ok(s), Ok(e)) = (start.parse::<Timestamp>(), end.parse::<Timestamp>()) else {
        return String::new();
    };
    span_text(e.duration_since(s))
}

/// A signed span as text: `-1m30s`, `2h0m0s`, `500ms`.
fn span_text(d: SignedDuration) -> String {
    let text = duration_text(d.unsigned_abs());
    if d.is_negative() { format!("-{text}") } else { text }
}

// ---- counts ----

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct CountInput {
    /// the log search, as datadog_logs takes it; empty counts every log of the window
    #[serde(default)]
    query: String,
    /// only the logs of one Remit chain (ch_…), as datadog_logs takes it
    #[serde(default)]
    chain: String,
    /// where the window begins: RFC3339 or relative such as -30m, -6h, -7d; default -1h
    #[serde(default)]
    from: String,
    /// where the window ends: RFC3339 or relative; default now
    #[serde(default)]
    to: String,
    /// count per value of this facet, such as service, status, host or @http.status_code; empty gives one total
    #[serde(default)]
    group_by: String,
    /// count per time bucket of this size, such as 5m or 1h; empty gives the window's total
    #[serde(default)]
    interval: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct CountResponse {
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    data: CountData,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    meta: CountMeta,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct CountData {
    buckets: Option<Vec<Bucket>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Bucket {
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    by: serde_json::Map<String, Value>,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    computes: serde_json::Map<String, Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct CountMeta {
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    status: String,
}

impl Datadog {
    async fn log_counts(&self, input: CountInput) -> Result<String> {
        let api = self.api()?;
        let (from, to) = self.window(&input.from, &input.to, "-1h")?;
        let mut compute = json!({"aggregation": "count", "type": "total"});
        if !input.interval.is_empty() {
            let ok = parse_duration(&input.interval).is_ok_and(|iv| iv >= SignedDuration::from_secs(1));
            if !ok {
                return fail!("interval {:?} is not a duration such as 5m or 1h", input.interval);
            }
            compute = json!({"aggregation": "count", "type": "timeseries", "interval": input.interval});
        }
        let mut body = json!({
            "compute": [compute],
            "filter": {"query": of_chain(&input.query, &input.chain)?, "from": rfc3339(from), "to": rfc3339(to)},
        });
        if !input.group_by.is_empty()
            && let Some(m) = body.as_object_mut()
        {
            m.insert(
                "group_by".to_owned(),
                json!([{"facet": input.group_by, "limit": COUNT_GROUPS, "sort": {"aggregation": "count", "order": "desc", "type": "measure"}}]),
            );
        }
        let r: CountResponse = decode(&api.post_json("/api/v2/logs/analytics/aggregate", &body).await?)?;
        Ok(count_lines(&r))
    }
}

/// A count laid out: one line per group, a bucket of a timeseries as time=value in order.
fn count_lines(r: &CountResponse) -> String {
    let buckets = r.data.buckets.as_deref().unwrap_or_default();
    let mut b = format!("log count, {} groups\n", buckets.len());
    if r.meta.status == "timeout" {
        b.push_str("[the count timed out and is partial: narrow the window or the query]\n");
    }
    for bucket in buckets {
        let label = if bucket.by.is_empty() {
            "total".to_owned()
        } else {
            labels_of(&bucket.by.iter().map(|(k, v)| (k.clone(), value_text_of(v))).collect())
        };
        b.push_str(&format!("{label} {}\n", count_value(bucket.computes.get("c0"))));
    }
    b.trim_end_matches('\n').to_owned()
}

/// A count (a number) or a timeseries (points of time and value) in one line.
fn count_value(v: Option<&Value>) -> String {
    let Some(Value::Array(points)) = v else {
        return format!("= {}", v.map_or_else(|| "<nil>".to_owned(), value_text_of));
    };
    let parts: Vec<String> = points
        .iter()
        .filter_map(Value::as_object)
        .map(|m| {
            format!(
                "{}={}",
                str_of(m.get("time")),
                m.get("value").map_or_else(|| "<nil>".to_owned(), value_text_of)
            )
        })
        .collect();
    parts.join(" ")
}

// ---- metrics ----

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct MetricsInput {
    /// the metric query, such as avg:system.cpu.user{service:api} or sum:trace.http.request.errors{env:prod}.as_count()
    query: String,
    /// where the window begins: RFC3339 or relative such as -30m, -6h, -7d; default -1h
    #[serde(default)]
    from: String,
    /// where the window ends: RFC3339 or relative; default now
    #[serde(default)]
    to: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MetricsResponse {
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    error: String,
    series: Option<Vec<Series>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Series {
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    expression: String,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    scope: String,
    pointlist: Option<Vec<Vec<Value>>>,
}

impl Datadog {
    async fn metrics(&self, input: MetricsInput) -> Result<String> {
        let api = self.api()?;
        if input.query.trim().is_empty() {
            return fail!("query is empty");
        }
        let (from, to) = self.window(&input.from, &input.to, "-1h")?;
        let q = query(&[
            ("query", &input.query),
            ("from", &from.as_second().to_string()),
            ("to", &to.as_second().to_string()),
        ]);
        let r: MetricsResponse = decode(&api.get("/api/v1/query", &q).await?)?;
        if !r.error.is_empty() {
            return fail!("datadog: {}", r.error);
        }
        let series = r.series.unwrap_or_default();
        let mut b = format!("{} series\n", series.len());
        for (i, s) in series.iter().enumerate() {
            if i == MAX_SERIES {
                b.push_str(&format!(
                    "[{} more series not shown; aggregate with sum by {{tag}} or add a tag to the scope]\n",
                    series.len().saturating_sub(MAX_SERIES)
                ));
                break;
            }
            let points = seconds_of(s.pointlist.as_deref().unwrap_or_default());
            b.push_str(&format!(
                "{} {{{}}} {}\n",
                s.expression,
                s.scope,
                range_summary(&points)
            ));
        }
        Ok(b.trim_end_matches('\n').to_owned())
    }
}

/// A Datadog pointlist, whose times are milliseconds, with its times in seconds, as `range_summary` reads them. A point
/// without a value (a gap) keeps its null and is skipped by the summary.
fn seconds_of(points: &[Vec<Value>]) -> Vec<Vec<Value>> {
    points
        .iter()
        .filter_map(|p| match p.as_slice() {
            [Value::Number(ms), v] => ms.as_f64().map(|ms| vec![json!(ms / 1000.0), v.clone()]),
            _ => None,
        })
        .collect()
}

// ---- monitors ----

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct MonitorsInput {
    /// free words match the monitor's name; facets narrow it: status:alert, status:warn, status:"no data", tag:service:api, type:metric; empty lists all monitors
    #[serde(default)]
    query: String,
    /// the page to read, from 0; default 0
    #[serde(default)]
    page: i64,
    /// monitors per page, 1 to 50; default 25
    #[serde(default)]
    per_page: i64,
}

#[derive(Debug, Serialize)]
struct MonitorRow {
    id: i64,
    name: String,
    status: String,
    #[serde(rename = "type")]
    kind: String,
    query: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    #[serde(rename = "lastTriggered", skip_serializing_if = "String::is_empty")]
    last_triggered_at: String,
}

/// A page of GET /api/v1/monitor/search; `status` is the monitor's overall state.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MonitorSearch {
    monitors: Option<Vec<FoundMonitor>>,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    metadata: SearchMeta,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FoundMonitor {
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    id: i64,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    name: String,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    status: String,
    #[serde(rename = "type")]
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    kind: String,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    query: String,
    tags: Option<Vec<String>>,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    last_triggered_ts: i64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SearchMeta {
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    total_count: i64,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    page_count: i64,
}

/// The overall states of a monitor that need a look, the most urgent first.
const NOT_OK: [&str; 3] = ["Alert", "Warn", "No Data"];

/// Orders monitors so the ones that need a look come first.
fn monitor_rank(status: &str) -> usize {
    NOT_OK.iter().position(|s| *s == status).unwrap_or(NOT_OK.len())
}

async fn search_monitors(api: &Api, q: &[(String, String)]) -> Result<MonitorSearch> {
    decode(&api.get("/api/v1/monitor/search", q).await?)
}

impl Datadog {
    async fn monitors(&self, input: MonitorsInput) -> Result<String> {
        if input.page < 0 || input.per_page < 0 || input.per_page > 50 {
            return fail!("page is from 0 and per_page is from 1 to 50");
        }
        let api = self.api()?;
        let per_page = if input.per_page == 0 { 25 } else { input.per_page };
        let mut q = query(&[("page", &input.page.to_string()), ("per_page", &per_page.to_string())]);
        if !input.query.trim().is_empty() {
            q.push(("query".to_owned(), input.query.clone()));
        }
        let r = search_monitors(&api, &q).await?;
        let mut list: Vec<MonitorRow> = r
            .monitors
            .unwrap_or_default()
            .into_iter()
            .map(|m| MonitorRow {
                last_triggered_at: Some(m.last_triggered_ts)
                    .filter(|ts| *ts > 0)
                    .and_then(|ts| Timestamp::from_second(ts).ok())
                    .map_or_else(String::new, rfc3339),
                id: m.id,
                name: m.name,
                status: m.status,
                kind: m.kind,
                query: m.query,
                tags: m.tags.unwrap_or_default(),
            })
            .collect();
        list.sort_by_key(|m| monitor_rank(&m.status));
        let out = render(&list)?;
        Ok(format!(
            "{} monitors match, page {} of {}\n{out}",
            r.metadata.total_count,
            input.page,
            r.metadata.page_count.max(1)
        ))
    }
}

// ---- poll: the monitors that are not OK ----

/// As datadog_monitors reads them.
const POLL_PER_PAGE: i64 = 50;
/// The most pages one poll searches: a query that matches more monitors is refused and says to narrow it.
const POLL_PAGES: i64 = 20;
/// The most monitors that are not OK one poll reads the state of, a GET each. A query that matches more is refused and says
/// to narrow it, rather than hearing some of them and missing others.
const POLL_MONITORS: usize = 50;

/// The part of GET /api/v1/monitor/{id} a poll reads. overall_state_modified, when the overall state last changed, is in the
/// answer though not in Datadog's published schema; it is null for a monitor whose state never changed, and then modified,
/// when it was last edited, stands for it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct MonitorState {
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    pub id: i64,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    pub name: String,
    #[serde(rename = "type")]
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    pub kind: String,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    pub query: String,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    pub message: String,
    pub tags: Option<Vec<String>>,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    pub overall_state: String,
    #[serde(rename = "overall_state_modified")]
    pub state_modified: Option<String>,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    pub modified: String,
}

impl MonitorState {
    /// When the monitor's overall state last changed; none when Datadog gave no time it can be read by.
    fn changed(&self) -> Option<Timestamp> {
        [
            self.state_modified.as_deref().unwrap_or_default(),
            self.modified.as_str(),
        ]
        .into_iter()
        .find_map(|s| s.parse::<Timestamp>().ok())
    }
}

/// A monitor that is not OK, with its id as an item names it and when its state changed.
struct Stamped {
    state: MonitorState,
    id: String,
    at: Timestamp,
}

/// What an event of a monitor carries: enough to decide and to open it, the message cut at POLL_TEXT_MAX.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PollBody {
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    pub id: i64,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    pub name: String,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    pub overall_state: String,
    /// When the overall state last changed.
    #[serde(rename = "changed_at")]
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    pub changed: String,
    #[serde(rename = "type")]
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    pub kind: String,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    pub query: String,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    pub tags: Vec<String>,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    pub message: String,
    #[serde(
        deserialize_with = "crate::null_json::null_as_zero",
        default,
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub message_cut: bool,
    #[serde(
        deserialize_with = "crate::null_json::null_as_zero",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub url: String,
}

/// The monitors that went into Alert, Warn or No Data since `cursor`, oldest change first. `query` is datadog_monitors'
/// search grammar (empty is every monitor), or a reading (datadog/reading.rs). An item's id is the monitor's id and its
/// version `<overall_state>@<when it changed>`, so a monitor that alerts, then warns, then alerts again is three events. A
/// monitor that recovers to OK is no item: it is heard again when it next leaves OK. An empty cursor answers no monitor and
/// marks the newest change among the ones not OK now (none: the zero stamp, after which every one that leaves OK is new).
impl Pollable for Datadog {
    fn poll<'a>(&'a self, query: &'a str, cursor: &'a str, limit: usize) -> BoxFuture<'a, Result<PollResult>> {
        Box::pin(async move {
            let r = self.poll_monitors(query, cursor, limit).await;
            without_secrets(r, &[self.api_key.clone(), self.app_key.clone()])
        })
    }
}

impl Datadog {
    async fn poll_monitors(&self, query: &str, cursor: &str, limit: usize) -> Result<PollResult> {
        if is_reading(query) {
            return self.poll_reading(query, cursor).await;
        }
        let mut mark = if cursor.is_empty() {
            PollMark::default()
        } else {
            PollMark::parse(cursor)?
        };
        let api = self.api()?;
        let states = self.not_ok(&api, query).await?;
        if cursor.is_empty() {
            for s in &states {
                mark.take(s.at, &s.id);
            }
            return Ok(PollResult {
                items: Vec::new(),
                cursor: mark.to_string(),
            });
        }
        let mut items = Vec::new();
        for s in &states {
            if mark.handed(s.at, &s.id) {
                continue;
            }
            items.push(self.poll_item(s)?);
            mark.take(s.at, &s.id);
            if items.len() == limit {
                break;
            }
        }
        Ok(PollResult {
            items,
            cursor: mark.to_string(),
        })
    }

    /// The monitors `query` matches whose overall state is not OK, oldest change first.
    async fn not_ok(&self, api: &Api, q: &str) -> Result<Vec<Stamped>> {
        let mut ids = Vec::new();
        let mut page: i64 = 0;
        loop {
            if page == POLL_PAGES {
                return fail!(
                    "datadog: more than {} monitors match {:?}: narrow the query, for example with tag:service:<name>",
                    POLL_PAGES.saturating_mul(POLL_PER_PAGE),
                    shorten(q, 80)
                );
            }
            let mut params = query(&[("page", &page.to_string()), ("per_page", &POLL_PER_PAGE.to_string())]);
            if !q.trim().is_empty() {
                params.push(("query".to_owned(), q.to_owned()));
            }
            let r = search_monitors(api, &params).await?;
            ids.extend(
                r.monitors
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|m| NOT_OK.contains(&m.status.as_str()))
                    .map(|m| m.id),
            );
            page = page.saturating_add(1);
            if page >= r.metadata.page_count {
                break;
            }
        }
        if ids.len() > POLL_MONITORS {
            return fail!(
                "datadog: {} monitors that match {:?} are not OK and a poll reads at most {POLL_MONITORS}: narrow the query, for example with status:alert or tag:service:<name>",
                ids.len(),
                shorten(q, 80)
            );
        }
        let mut out = Vec::new();
        for id in ids {
            let raw = match api.get(&format!("/api/v1/monitor/{id}"), &[]).await {
                Err(e) if e.status().is_some_and(|s| s.code == 404) => continue, // deleted since the search
                other => other?,
            };
            let m: MonitorState =
                serde_json::from_slice(&raw).map_err(|e| err!("datadog: the monitor is not JSON: {e}"))?;
            // Back to OK since the search, or without a time it changed at.
            let Some(at) = m.changed().filter(|_| NOT_OK.contains(&m.overall_state.as_str())) else {
                continue;
            };
            out.push(Stamped {
                id: m.id.to_string(),
                state: m,
                at,
            });
        }
        out.sort_by(|a, b| a.at.cmp(&b.at).then(a.state.id.cmp(&b.state.id)));
        Ok(out)
    }

    fn poll_item(&self, s: &Stamped) -> Result<PollItem> {
        let m = &s.state;
        let text = redact_text(&m.message);
        let message = cut_bytes(&text, POLL_TEXT_MAX);
        let stamp = rfc3339_nano(s.at);
        let raw = render(&PollBody {
            id: m.id,
            name: m.name.clone(),
            overall_state: m.overall_state.clone(),
            changed: stamp.clone(),
            kind: m.kind.clone(),
            query: m.query.clone(),
            tags: m.tags.clone().unwrap_or_default(),
            message: message.to_owned(),
            message_cut: message.len() < text.len(),
            url: self.monitor_url(&s.id),
        })?;
        Ok(PollItem {
            id: s.id.clone(),
            version: format!("{}@{stamp}", m.overall_state),
            body: raw,
        })
    }

    /// The monitor's page in the organisation's Datadog app: app.<site> for a region's first site (datadoghq.com,
    /// datadoghq.eu, ddog-gov.com), the site itself for the others (us5.datadoghq.com, ap1.datadoghq.com).
    fn monitor_url(&self, id: &str) -> String {
        if !SITE.is_match(&self.site) {
            return String::new();
        }
        let host = if self.site.matches('.').count() == 1 {
            format!("app.{}", self.site)
        } else {
            self.site.clone()
        };
        format!("https://{host}/monitors/{id}")
    }
}

#[cfg(test)]
mod tests {
    use super::of_chain;

    #[test]
    fn a_chain_narrows_the_search_and_cannot_widen_it() {
        assert_eq!(
            of_chain("status:error", "ch_abc-1").unwrap(),
            "@remit.chain_id:ch_abc-1 (status:error)"
        );
        assert_eq!(
            of_chain("status:error OR service:billing", "ch_abc").unwrap(),
            "@remit.chain_id:ch_abc (status:error OR service:billing)",
            "an OR is inside the group, so it stays inside the chain"
        );
        assert_eq!(of_chain("", "ch_abc").unwrap(), "@remit.chain_id:ch_abc");
        assert_eq!(of_chain("status:error", "").unwrap(), "status:error");
        assert!(of_chain("", "ch_a OR service:*").is_err(), "a chain id is not a search");
    }
}
