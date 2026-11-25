//! Amplitude is the product analytics of the owner's app: which events fire, how many users do, what a funnel converts, and
//! what one user did. It reads the Dashboard REST API (https://amplitude.com/docs/apis/analytics/dashboard-rest), which only
//! reads. Credentials: AMPLITUDE_API_KEY and AMPLITUDE_SECRET_KEY (HTTP basic); AMPLITUDE_API_URL is optional and is
//! https://analytics.eu.amplitude.com for a project in the EU.

use std::collections::BTreeMap;
use std::future::Future;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use plugin_sdk::{Call, Definition, Effect, Error, Tool, Var};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use source::{
    Api, CAP_CUSTOMERS, Http, Query, Shared, mask_emails, null_as_zero, parse_when, render, shorten, table,
    value_text_of,
};

const NAME: &str = "Amplitude";
const DEFAULT_URL: &str = "https://amplitude.com";
/// The longest period one query reads: a wider one is slow and, at a daily interval, rarely what was meant.
const MAX_DAYS: i32 = 366;
/// How many of the latest buckets of a series are listed one by one; the rest is summarised.
const RECENT: usize = 14;
/// How many groups of one segmentation are shown.
const MAX_SERIES: usize = 20;
/// How many event types `amplitude_events` lists.
const MAX_EVENTS: usize = 100;
/// How many users `amplitude_user_search` lists.
const MAX_MATCHES: usize = 20;
/// How many properties of one event or one user are shown, and how long a value is.
const MAX_PROPS: usize = 10;
const PROP_WIDTH: usize = 100;

const RATE_LIMITED: &str = "amplitude refused the call because the Dashboard API is rate limited (HTTP 429): it allows only a few queries at once and a fixed number per hour. Wait a few minutes, then ask once with a wider period or fewer groups instead of many small calls";

/// One call's view of Amplitude: the connection's credentials and the time the call began.
struct Amplitude {
    now: Timestamp,
    http: Http,
    key: String,
    secret: String,
    url: String,
}

impl Amplitude {
    fn new(call: &Call, shared: &Shared) -> Amplitude {
        let base = call.var("AMPLITUDE_API_URL").trim();
        Amplitude {
            now: shared.now.now(),
            http: shared.http.clone(),
            key: call.var("AMPLITUDE_API_KEY").to_owned(),
            secret: call.var("AMPLITUDE_SECRET_KEY").to_owned(),
            url: if base.is_empty() { DEFAULT_URL } else { base }.to_owned(),
        }
    }

    /// The answer to GET `path`, with the two failures an agent can act on in plain words: Amplitude limits how many dashboard
    /// queries run at once and per hour, and a misspelt event type is refused with a code.
    async fn get(&self, path: &str, query: &[(String, String)]) -> Result<Vec<u8>, Error> {
        let credential = STANDARD.encode(format!("{}:{}", self.key, self.secret));
        let api = Api {
            name: NAME.to_owned(),
            base: self.url.clone(),
            headers: vec![("Authorization".to_owned(), format!("Basic {credential}"))],
            http: self.http.clone(),
        };
        match api.get(path, query).await {
            Ok(raw) => Ok(raw),
            Err(e) if e.status().is_some_and(|s| s.code == 429) => Err(RATE_LIMITED.into()),
            Err(e) if e.to_string().contains("event_type_not_found") => {
                Err(format!("{e} (amplitude_events lists the exact event names)").into())
            }
            Err(e) => Err(e.into()),
        }
    }
}

/// The plugin on the clock and client of `shared`: `plugin_sdk` calls it with the credentials of the connection a tool was called through.
pub(crate) fn plugin(shared: Shared) -> Definition {
    Definition {
        label: NAME.into(),
        capability: CAP_CUSTOMERS.into(),
        vars: vec![
            Var {
                name: "AMPLITUDE_API_KEY".into(),
                what: "the API key of the Amplitude project (Settings, Projects)".into(),
                secret: true,
                ..Default::default()
            },
            Var {
                name: "AMPLITUDE_SECRET_KEY".into(),
                what: "the secret key of the same project".into(),
                secret: true,
                ..Default::default()
            },
            Var {
                name: "AMPLITUDE_API_URL".into(),
                what: "the API address, only when the project is in the EU data center (https://analytics.eu.amplitude.com)".into(),
                optional: true,
                address: true,
                ..Default::default()
            },
        ],
        tools: vec![
            tool(&shared, "events", "List the event types the product sends to Amplitude, busiest first, with Amplitude's own recent volume and whether the event is still active. Start here to learn the exact event names the other amplitude tools take.", Amplitude::events),
            tool(&shared, "segmentation", "How often an event happened over a period: distinct users (uniques) or event count (totals) per day, week, month or hour, optionally split by one property such as platform, country or version. Answers 'is this feature used', 'did this event drop' and 'which version stopped sending it'. Each series is summarised (the whole-period value, min, max, average and the latest buckets), not dumped.", Amplitude::segmentation),
            tool(&shared, "funnel", "How many users get through a sequence of 2 to 10 events in order, with the share that reaches each step from the first and from the step before. Use it to find where users drop out of a flow.", Amplitude::funnel),
            tool(&shared, "retention", "How many of the users who did a starting event come back and do a return event, for each period after (day 0, 1, 2...). Use it to see whether users keep using a feature.", Amplitude::retention),
            tool(&shared, "user_search", "Find a user by user id, device id or Amplitude id (or the start of one) and get the Amplitude id that amplitude_user_activity takes. Emails in an answer are masked.", Amplitude::user_search),
            tool(&shared, "user_activity", "What one user did: their profile (first and last seen, events, sessions, country, platform, properties) and their latest events, newest first, with the event properties that show the page or error. Use it to see what a customer did before something went wrong. Emails in an answer are masked.", Amplitude::user_activity),
        ],
        ..Default::default()
    }
}

/// A read tool whose handler is a method of `Amplitude`, built for each call from the connection's variables.
fn tool<In, F, Fut>(shared: &Shared, name: &str, description: &str, run: F) -> Tool
where
    In: DeserializeOwned + JsonSchema + Send + 'static,
    F: Fn(Amplitude, In) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<String, Error>> + Send + 'static,
{
    let shared = shared.clone();
    Tool::new(name, description, Effect::Read, move |call: Call, input: In| {
        run(Amplitude::new(&call, &shared), input)
    })
}

/// `v` as the agent reads it: credential fields dropped, customer emails masked.
fn reply(v: &impl Serialize) -> Result<String, Error> {
    Ok(mask_emails(&render(v)?))
}

fn decode<T: DeserializeOwned>(raw: &[u8], what: &str) -> Result<T, Error> {
    serde_json::from_slice(raw).map_err(|e| format!("amplitude: the {what} is not JSON: {e}").into())
}

// --- time ---

/// The days a query reads, first and last inclusive, in UTC.
#[derive(Debug, Clone, Copy)]
struct Days {
    start: Date,
    end: Date,
}

impl Days {
    fn query(self) -> Query {
        vec![
            ("start".to_owned(), compact(self.start)),
            ("end".to_owned(), compact(self.end)),
        ]
    }
}

fn compact(d: Date) -> String {
    d.strftime("%Y%m%d").to_string()
}

fn utc_day(t: Timestamp) -> Date {
    t.to_zoned(TimeZone::UTC).date()
}

/// The days from `start` to `end`, as an agent wrote them: a date (2026-10-30 or 20261030), RFC3339 or relative to now (-7d).
/// An empty start is a week ago and an empty end is now.
fn window(start: &str, end: &str, now: Timestamp) -> Result<Days, String> {
    let start = if start.trim().is_empty() { "-7d" } else { start };
    let from = parse_day(start, now).map_err(|e| format!("start: {e}"))?;
    let to = parse_day(end, now).map_err(|e| format!("end: {e}"))?;
    if to < from {
        return Err("end is before start".to_owned());
    }
    let days = from.until(to).map_err(|e| e.to_string())?.get_days();
    if days > MAX_DAYS {
        return Err(format!("the period is longer than {MAX_DAYS} days: narrow it"));
    }
    Ok(Days { start: from, end: to })
}

/// `parse_when` that also takes a bare date, the form an agent reads in Amplitude's own screens.
fn parse_day(s: &str, now: Timestamp) -> Result<Date, String> {
    let s = s.trim();
    if let Some(d) = date_of(s) {
        return Ok(d);
    }
    parse_when(s, now)
        .map(utc_day)
        .map_err(|_| format!("{s:?} is not a date: use 2026-10-30, 20261030 or relative to now such as -7d"))
}

/// The date `s` writes as 2026-10-30 or 20261030.
fn date_of(s: &str) -> Option<Date> {
    let (y, m, d) = match s.len() {
        10 if s.get(4..5) == Some("-") && s.get(7..8) == Some("-") => (s.get(0..4)?, s.get(5..7)?, s.get(8..10)?),
        8 => (s.get(0..4)?, s.get(4..6)?, s.get(6..8)?),
        _ => return None,
    };
    if ![y, m, d].iter().all(|p| p.bytes().all(|b| b.is_ascii_digit())) {
        return None;
    }
    Date::new(y.parse().ok()?, m.parse().ok()?, d.parse().ok()?).ok()
}

/// The `i` Amplitude takes for a bucket size. Retention reads days, weeks and months only.
fn interval_code(name: &str, hourly: bool) -> Result<&'static str, String> {
    match name.trim().to_lowercase().as_str() {
        "" | "day" => return Ok("1"),
        "week" => return Ok("7"),
        "month" => return Ok("30"),
        "hour" if hourly => return Ok("-3600000"),
        _ => {}
    }
    if hourly {
        return Err(format!("interval {name:?} is not one of day, week, month, hour"));
    }
    Err(format!("interval {name:?} is not one of day, week, month"))
}

fn interval_name(code: &str) -> &'static str {
    match code {
        "7" => "week",
        "30" => "month",
        "-3600000" => "hour",
        _ => "day",
    }
}

/// The JSON Amplitude takes for an event (`e`, `se`, `re`): built by encoding, so a name never changes the query.
fn event_param(name: &str) -> Result<String, Error> {
    Ok(serde_json::to_string(&BTreeMap::from([("event_type", name)]))?)
}

/// A share from 0 to 1 as a percentage with one decimal.
fn pct(share: f64) -> f64 {
    (share * 1000.0).round() / 10.0
}

// --- events ---

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EventsInput {
    /// only event types whose name contains this word, any case
    #[serde(default)]
    query: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct EventType {
    #[serde(deserialize_with = "null_as_zero")]
    value: String,
    #[serde(deserialize_with = "null_as_zero")]
    display: String,
    #[serde(deserialize_with = "null_as_zero")]
    totals: f64,
    #[serde(deserialize_with = "null_as_zero")]
    hidden: bool,
    #[serde(deserialize_with = "null_as_zero")]
    deleted: bool,
    #[serde(deserialize_with = "null_as_zero")]
    non_active: bool,
}

impl Amplitude {
    async fn events(self, input: EventsInput) -> Result<String, Error> {
        #[derive(Deserialize)]
        struct List {
            #[serde(default, deserialize_with = "null_as_zero")]
            data: Vec<EventType>,
        }
        let raw = self.get("/api/2/events/list", &[]).await?;
        let list: List = decode(&raw, "event list")?;
        let word = input.query.trim().to_lowercase();
        let mut found: Vec<EventType> = list
            .data
            .into_iter()
            .filter(|e| !e.deleted && format!("{} {}", e.value, e.display).to_lowercase().contains(&word))
            .collect();
        if found.is_empty() {
            return Ok("No event type matches.".to_owned());
        }
        // Busiest first: an event with no volume is one the product stopped sending.
        found.sort_by(|p, q| q.totals.total_cmp(&p.totals));
        let rows: Vec<Vec<String>> = found
            .iter()
            .map(|e| {
                let shown = if e.display == e.value {
                    String::new()
                } else {
                    e.display.clone()
                };
                let mut flags = Vec::new();
                if e.non_active {
                    flags.push("not active");
                }
                if e.hidden {
                    flags.push("hidden");
                }
                vec![e.value.clone(), shown, e.totals.to_string(), flags.join(", ")]
            })
            .collect();
        let cols = ["event", "shown as", "recent volume", "flags"].map(str::to_owned);
        Ok(mask_emails(&table(&cols, &rows, MAX_EVENTS)))
    }
}

// --- segmentation ---

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SegmentationInput {
    /// the exact event type name, from amplitude_events; _active counts any activity
    event: String,
    /// first day: 2026-10-23, 20261023 or relative such as -7d; default -7d
    #[serde(default)]
    start: String,
    /// last day, same forms; default today
    #[serde(default)]
    end: String,
    /// bucket size: day (default), week, month or hour (hour needs a short period)
    #[serde(default)]
    interval: String,
    /// uniques (distinct users, default) or totals (number of events)
    #[serde(default)]
    metric: String,
    /// split by one property: platform, country, version, os, device, language, city, or gp:<name> for a custom user property
    #[serde(default)]
    group_by: String,
    /// most groups when group_by is set, 1 to 50; default 10
    #[serde(default)]
    limit: i64,
}

struct SegmentationRequest {
    event: String,
    metric: String,
    group: String,
    interval: &'static str,
    limit: i64,
    days: Days,
}

impl SegmentationRequest {
    fn of(input: SegmentationInput, now: Timestamp) -> Result<SegmentationRequest, String> {
        let event = input.event.trim().to_owned();
        if event.is_empty() {
            return Err("event is empty: amplitude_events lists the event names".to_owned());
        }
        let days = window(&input.start, &input.end, now)?;
        let interval = interval_code(&input.interval, true)?;
        let metric = match input.metric.trim().to_lowercase().as_str() {
            "" => "uniques".to_owned(),
            m @ ("uniques" | "totals") => m.to_owned(),
            _ => return Err(format!("metric {:?} is not uniques or totals", input.metric)),
        };
        if !(0..=50).contains(&input.limit) {
            return Err("limit is between 1 and 50".to_owned());
        }
        Ok(SegmentationRequest {
            event,
            metric,
            group: input.group_by.trim().to_owned(),
            interval,
            limit: if input.limit == 0 { 10 } else { input.limit },
            days,
        })
    }

    fn query(&self) -> Result<Query, Error> {
        let mut q = self.days.query();
        q.push(("e".to_owned(), event_param(&self.event)?));
        q.push(("m".to_owned(), self.metric.clone()));
        q.push(("i".to_owned(), self.interval.to_owned()));
        if !self.group.is_empty() {
            q.push(("g".to_owned(), self.group.clone()));
            q.push(("limit".to_owned(), self.limit.to_string()));
        }
        Ok(q)
    }
}

impl Amplitude {
    async fn segmentation(self, input: SegmentationInput) -> Result<String, Error> {
        let req = SegmentationRequest::of(input, self.now)?;
        let raw = self.get("/api/2/events/segmentation", &req.query()?).await?;
        summarise_segmentation(&raw, &req, self.now)
    }
}

#[derive(Debug, Serialize)]
struct SegmentationAnswer {
    event: String,
    metric: String,
    interval: String,
    from: String,
    to: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    note: String,
    series: Vec<Series>,
    #[serde(skip_serializing_if = "String::is_empty")]
    more: String,
}

#[derive(Debug, Default, Serialize)]
struct Series {
    label: String,
    period: f64,
    min: f64,
    min_at: String,
    max: f64,
    max_at: String,
    avg: f64,
    recent: String,
}

/// Amplitude's series reduced to one object per group. `period` is Amplitude's value for the whole period: for uniques it
/// counts each user once, which the sum of the buckets does not.
fn summarise_segmentation(raw: &[u8], req: &SegmentationRequest, now: Timestamp) -> Result<String, Error> {
    #[derive(Default, Deserialize)]
    #[serde(default)]
    struct Collapsed {
        #[serde(deserialize_with = "null_as_zero")]
        value: f64,
    }
    #[derive(Default, Deserialize)]
    #[serde(default, rename_all = "camelCase")]
    struct Data {
        #[serde(deserialize_with = "null_as_zero")]
        series: Vec<Vec<Option<f64>>>,
        #[serde(deserialize_with = "null_as_zero")]
        series_collapsed: Vec<Vec<Collapsed>>,
        #[serde(deserialize_with = "null_as_zero")]
        series_labels: Vec<Value>,
        #[serde(deserialize_with = "null_as_zero")]
        x_values: Vec<String>,
    }
    #[derive(Deserialize)]
    struct Answer {
        #[serde(default, deserialize_with = "null_as_zero")]
        data: Data,
    }
    let r: Answer = decode(raw, "segmentation")?;
    let mut ans = SegmentationAnswer {
        event: req.event.clone(),
        metric: req.metric.clone(),
        interval: interval_name(req.interval).to_owned(),
        from: req.days.start.to_string(),
        to: req.days.end.to_string(),
        note: String::new(),
        series: Vec::new(),
        more: String::new(),
    };
    if req.days.end >= utc_day(now) {
        ans.note = "the last bucket reaches now and is still filling: do not read it as a drop".to_owned();
    }
    for (i, values) in r.data.series.iter().enumerate() {
        if i == MAX_SERIES {
            ans.more = format!(
                "{} more groups not shown; narrow with a shorter period or a smaller limit",
                r.data.series.len().saturating_sub(MAX_SERIES)
            );
            break;
        }
        let values: Vec<f64> = values.iter().map(|v| v.unwrap_or_default()).collect();
        let mut s = summarise_series(&values, &r.data.x_values);
        s.label = match r.data.series_labels.get(i) {
            Some(label) if !req.group.is_empty() => value_text_of(label),
            _ => "all".to_owned(),
        };
        if let Some(c) = r.data.series_collapsed.get(i).and_then(|c| c.first()) {
            s.period = c.value;
        }
        ans.series.push(s);
    }
    reply(&ans)
}

fn summarise_series(values: &[f64], xs: &[String]) -> Series {
    let mut s = Series::default();
    if values.is_empty() {
        return s;
    }
    (s.min, s.max) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut sum = 0.0;
    for (i, v) in values.iter().copied().enumerate() {
        sum += v;
        if v < s.min {
            (s.min, s.min_at) = (v, at(xs, i));
        }
        if v > s.max {
            (s.max, s.max_at) = (v, at(xs, i));
        }
    }
    let n = f64::from(u32::try_from(values.len()).unwrap_or(u32::MAX));
    s.avg = (sum / n * 100.0).round() / 100.0;
    let first = values.len().saturating_sub(RECENT);
    s.recent = values
        .iter()
        .enumerate()
        .skip(first)
        .map(|(i, v)| format!("{}={v}", at(xs, i)))
        .collect::<Vec<_>>()
        .join(" ");
    s
}

fn at(xs: &[String], i: usize) -> String {
    xs.get(i).cloned().unwrap_or_else(|| format!("#{i}"))
}

// --- funnel ---

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct FunnelInput {
    /// 2 to 10 exact event type names, in the order users should do them
    events: Vec<String>,
    /// first day: 2026-10-23, 20261023 or relative such as -7d; default -7d
    #[serde(default)]
    start: String,
    /// last day, same forms; default today
    #[serde(default)]
    end: String,
    /// ordered (default: in that order, other events allowed between), unordered, or sequential (one right after the other)
    #[serde(default)]
    mode: String,
}

#[derive(Debug, Default, Serialize)]
struct FunnelStep {
    event: String,
    users: f64,
    of_first_pct: f64,
    of_previous_pct: f64,
    dropped: f64,
}

impl Amplitude {
    async fn funnel(self, input: FunnelInput) -> Result<String, Error> {
        if !(2..=10).contains(&input.events.len()) {
            return Err("events needs 2 to 10 event type names".into());
        }
        let mut q = Query::new();
        for name in &input.events {
            if name.trim().is_empty() {
                return Err("an event name is empty: amplitude_events lists the event names".into());
            }
            q.push(("e".to_owned(), event_param(name.trim())?));
        }
        let mode = match input.mode.trim().to_lowercase().as_str() {
            "" => "ordered".to_owned(),
            m @ ("ordered" | "unordered" | "sequential") => m.to_owned(),
            _ => return Err(format!("mode {:?} is not ordered, unordered or sequential", input.mode).into()),
        };
        let w = window(&input.start, &input.end, self.now)?;
        q.extend(w.query());
        q.push(("mode".to_owned(), mode.clone()));
        let raw = self.get("/api/2/funnels", &q).await?;
        summarise_funnel(&raw, w, &mode)
    }
}

fn summarise_funnel(raw: &[u8], w: Days, mode: &str) -> Result<String, Error> {
    #[derive(Default, Deserialize)]
    #[serde(default, rename_all = "camelCase")]
    struct Funnel {
        #[serde(deserialize_with = "null_as_zero")]
        events: Vec<String>,
        #[serde(deserialize_with = "null_as_zero")]
        cumulative_raw: Vec<f64>,
        #[serde(deserialize_with = "null_as_zero")]
        cumulative: Vec<f64>,
        #[serde(deserialize_with = "null_as_zero")]
        step_by_step: Vec<f64>,
    }
    #[derive(Deserialize)]
    struct Answer {
        #[serde(default, deserialize_with = "null_as_zero")]
        data: Vec<Funnel>,
    }
    let r: Answer = decode(raw, "funnel")?;
    let f = r.data.first().ok_or("amplitude returned no funnel for these events")?;
    let mut steps: Vec<FunnelStep> = Vec::with_capacity(f.events.len());
    for (i, name) in f.events.iter().enumerate() {
        let users = f.cumulative_raw.get(i).copied().unwrap_or_default();
        let dropped = steps.last().map_or(0.0, |prev| prev.users - users);
        steps.push(FunnelStep {
            event: name.clone(),
            users,
            of_first_pct: f.cumulative.get(i).copied().map_or(0.0, pct),
            of_previous_pct: f.step_by_step.get(i).copied().map_or(0.0, pct),
            dropped,
        });
    }
    reply(&json!({"mode": mode, "from": w.start.to_string(), "to": w.end.to_string(), "steps": steps}))
}

// --- retention ---

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RetentionInput {
    /// the exact event type that puts a user in the group, from amplitude_events; _new counts new users
    start_event: String,
    /// the exact event type that counts as coming back; _active counts any activity
    return_event: String,
    /// first day of the groups: 2026-10-01, 20261001 or relative such as -14d; default -7d
    #[serde(default)]
    start: String,
    /// last day, same forms; default today
    #[serde(default)]
    end: String,
    /// period size: day (default), week or month
    #[serde(default)]
    interval: String,
}

#[derive(Debug, Serialize)]
struct RetentionPeriod {
    period: usize,
    returned: f64,
    out_of: f64,
    pct: f64,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    incomplete: bool,
}

impl Amplitude {
    async fn retention(self, input: RetentionInput) -> Result<String, Error> {
        let (start, ret) = (input.start_event.trim(), input.return_event.trim());
        if start.is_empty() || ret.is_empty() {
            return Err("start_event and return_event are both needed: amplitude_events lists the event names".into());
        }
        let interval = interval_code(&input.interval, false)?;
        let w = window(&input.start, &input.end, self.now)?;
        let mut q = w.query();
        q.push(("se".to_owned(), event_param(start)?));
        q.push(("re".to_owned(), event_param(ret)?));
        q.push(("i".to_owned(), interval.to_owned()));
        let raw = self.get("/api/2/retention", &q).await?;
        summarise_retention(&raw, w, interval_name(interval))
    }
}

/// The `combined` row Amplitude gives: all the groups of the period together, one entry per period after the start (0 is
/// the start itself).
fn summarise_retention(raw: &[u8], w: Days, interval: &str) -> Result<String, Error> {
    #[derive(Default, Deserialize)]
    #[serde(default)]
    struct Combined {
        #[serde(deserialize_with = "null_as_zero")]
        count: f64,
        #[serde(deserialize_with = "null_as_zero")]
        outof: f64,
        #[serde(deserialize_with = "null_as_zero")]
        incomplete: bool,
    }
    #[derive(Default, Deserialize)]
    #[serde(default)]
    struct One {
        #[serde(deserialize_with = "null_as_zero")]
        combined: Vec<Combined>,
    }
    #[derive(Default, Deserialize)]
    #[serde(default)]
    struct Data {
        #[serde(deserialize_with = "null_as_zero")]
        series: Vec<One>,
    }
    #[derive(Deserialize)]
    struct Answer {
        #[serde(default, deserialize_with = "null_as_zero")]
        data: Data,
    }
    let r: Answer = decode(raw, "retention")?;
    let first = r
        .data
        .series
        .first()
        .ok_or("amplitude returned no retention for these events")?;
    let rows: Vec<RetentionPeriod> = first
        .combined
        .iter()
        .enumerate()
        .map(|(i, c)| RetentionPeriod {
            period: i,
            returned: c.count,
            out_of: c.outof,
            pct: if c.outof > 0.0 { pct(c.count / c.outof) } else { 0.0 },
            incomplete: c.incomplete,
        })
        .collect();
    reply(
        &json!({"interval": interval, "from": w.start.to_string(), "to": w.end.to_string(),
        "note": "incomplete: some groups have not lived through that period yet", "periods": rows}),
    )
}

// --- users ---

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct UserSearchInput {
    /// a user id, device id or Amplitude id, or the start of one
    user: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct Match {
    #[serde(deserialize_with = "null_as_zero")]
    user_id: String,
    #[serde(deserialize_with = "null_as_zero")]
    amplitude_id: i64,
    #[serde(deserialize_with = "null_as_zero", skip_serializing_if = "String::is_empty")]
    platform: String,
    #[serde(deserialize_with = "null_as_zero", skip_serializing_if = "String::is_empty")]
    country: String,
    #[serde(deserialize_with = "null_as_zero", skip_serializing_if = "String::is_empty")]
    last_seen: String,
}

impl Amplitude {
    async fn user_search(self, input: UserSearchInput) -> Result<String, Error> {
        #[derive(Deserialize)]
        struct Answer {
            #[serde(default, deserialize_with = "null_as_zero")]
            matches: Vec<Match>,
        }
        let user = input.user.trim();
        if user.is_empty() {
            return Err("user is empty".into());
        }
        let raw = self
            .get("/api/2/usersearch", &[("user".to_owned(), user.to_owned())])
            .await?;
        let mut r: Answer = decode(&raw, "user search")?;
        if r.matches.is_empty() {
            return Ok("No user matches.".to_owned());
        }
        let mut ans = Map::new();
        if r.matches.len() > MAX_MATCHES {
            let more = r.matches.len().saturating_sub(MAX_MATCHES);
            r.matches.truncate(MAX_MATCHES);
            ans.insert(
                "more".to_owned(),
                format!("{more} more users not shown; give more of the id").into(),
            );
        }
        ans.insert("matches".to_owned(), serde_json::to_value(&r.matches)?);
        reply(&ans)
    }
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct UserActivityInput {
    /// the user's Amplitude id, from amplitude_user_search
    amplitude_id: i64,
    /// how many events, 1 to 100; default 25
    #[serde(default)]
    limit: i64,
    /// events to skip, newest first, to read further back; default 0
    #[serde(default)]
    offset: i64,
}

#[derive(Debug, Serialize)]
struct UserProfile {
    #[serde(skip_serializing_if = "String::is_empty")]
    user_id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    country: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    platform: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    version: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    paying: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    first_used: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    last_used: String,
    events: i64,
    sessions: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    properties: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Serialize)]
struct ActivityEvent {
    at: String,
    r#type: String,
    #[serde(skip_serializing_if = "is_zero")]
    session: i64,
    #[serde(skip_serializing_if = "String::is_empty")]
    platform: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    props: Option<BTreeMap<String, String>>,
}

fn is_zero(n: &i64) -> bool {
    *n == 0
}

impl Amplitude {
    async fn user_activity(self, input: UserActivityInput) -> Result<String, Error> {
        if input.amplitude_id <= 0 {
            return Err("amplitude_id is needed: amplitude_user_search finds it".into());
        }
        if !(0..=100).contains(&input.limit) || input.offset < 0 {
            return Err("limit is between 1 and 100 and offset is not negative".into());
        }
        let limit = if input.limit == 0 { 25 } else { input.limit };
        let mut q = vec![
            ("user".to_owned(), input.amplitude_id.to_string()),
            ("limit".to_owned(), limit.to_string()),
        ];
        if input.offset > 0 {
            q.push(("offset".to_owned(), input.offset.to_string()));
        }
        let raw = self.get("/api/2/useractivity", &q).await?;
        summarise_activity(&raw, limit, input.offset)
    }
}

/// What an investigation reads: who the user is, and for each event when, what, where and the properties that name a page
/// or an error. Network addresses, device ids and coordinates stay out.
fn summarise_activity(raw: &[u8], limit: i64, offset: i64) -> Result<String, Error> {
    #[derive(Default, Deserialize)]
    #[serde(default)]
    struct Event {
        #[serde(deserialize_with = "null_as_zero")]
        event_time: String,
        #[serde(deserialize_with = "null_as_zero")]
        event_type: String,
        #[serde(deserialize_with = "null_as_zero")]
        session_id: i64,
        #[serde(deserialize_with = "null_as_zero")]
        platform: String,
        #[serde(deserialize_with = "null_as_zero")]
        version_name: String,
        #[serde(deserialize_with = "null_as_zero")]
        event_properties: Map<String, Value>,
    }
    #[derive(Default, Deserialize)]
    #[serde(default)]
    struct UserData {
        #[serde(deserialize_with = "null_as_zero")]
        user_id: String,
        #[serde(deserialize_with = "null_as_zero")]
        country: String,
        #[serde(deserialize_with = "null_as_zero")]
        platform: String,
        #[serde(deserialize_with = "null_as_zero")]
        version: String,
        #[serde(deserialize_with = "null_as_zero")]
        paying: String,
        #[serde(deserialize_with = "null_as_zero")]
        first_used: String,
        #[serde(deserialize_with = "null_as_zero")]
        last_used: String,
        #[serde(deserialize_with = "null_as_zero")]
        num_events: i64,
        #[serde(deserialize_with = "null_as_zero")]
        num_sessions: i64,
        #[serde(deserialize_with = "null_as_zero")]
        properties: Map<String, Value>,
    }
    #[derive(Default, Deserialize)]
    #[serde(default, rename_all = "camelCase")]
    struct Answer {
        #[serde(deserialize_with = "null_as_zero")]
        events: Vec<Event>,
        #[serde(deserialize_with = "null_as_zero")]
        user_data: UserData,
    }
    let r: Answer = decode(raw, "user activity")?;
    if r.events.is_empty() && r.user_data.user_id.is_empty() && r.user_data.num_events == 0 {
        return Ok("No events for this user (the Amplitude id is unknown, or the user has none).".to_owned());
    }
    let u = r.user_data;
    let user = UserProfile {
        user_id: u.user_id,
        country: u.country,
        platform: u.platform,
        version: u.version,
        paying: u.paying,
        first_used: u.first_used,
        last_used: u.last_used,
        events: u.num_events,
        sessions: u.num_sessions,
        properties: props(&u.properties),
    };
    let events: Vec<ActivityEvent> = r
        .events
        .into_iter()
        .map(|e| ActivityEvent {
            props: props(&e.event_properties),
            at: e.event_time,
            r#type: e.event_type,
            session: e.session_id,
            platform: e.platform,
            version: e.version_name,
        })
        .collect();
    let mut ans = Map::new();
    if i64::try_from(events.len()).is_ok_and(|n| n == limit) {
        ans.insert(
            "more".to_owned(),
            format!("older events: call again with offset {}", offset.saturating_add(limit)).into(),
        );
    }
    ans.insert("user".to_owned(), serde_json::to_value(&user)?);
    ans.insert("events".to_owned(), serde_json::to_value(&events)?);
    reply(&ans)
}

/// `props` reduced to short text: empty values and Experiment flags are dropped, the properties an agent set come before the
/// ones the SDK adds, and at most `MAX_PROPS` are kept. A credential field loses its value in `reply` (`render`).
fn props(props: &Map<String, Value>) -> Option<BTreeMap<String, String>> {
    let mut keys: Vec<&String> = props
        .iter()
        .filter(|(k, v)| !k.starts_with("[Experiment]") && !empty_prop(v))
        .map(|(k, _)| k)
        .collect();
    keys.sort_by(|a, b| a.starts_with('[').cmp(&b.starts_with('[')).then_with(|| a.cmp(b)));
    keys.truncate(MAX_PROPS);
    if keys.is_empty() {
        return None;
    }
    Some(
        keys.into_iter()
            .map(|k| {
                let value = props.get(k).map(prop_value).unwrap_or_default();
                (k.clone(), shorten(&value, PROP_WIDTH))
            })
            .collect(),
    )
}

fn empty_prop(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        _ => false,
    }
}

fn prop_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
