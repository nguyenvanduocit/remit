//! Grafana is the monitoring stack's front: its Prometheus-compatible datasource answers PromQL, its alert rules say what is
//! firing, its dashboards say which metrics the owner already watches. Credentials: GRAFANA_URL and either GRAFANA_TOKEN (a
//! service account's) or GRAFANA_USER with GRAFANA_PASSWORD.

mod reading;

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use base64::Engine as _;
use jiff::{SignedDuration, SignedDurationRound, Timestamp, Unit};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::api::{Api, Http, Query};
use crate::env::{Clock, Env, missing};
use crate::error::{BoxFuture, Result, err, fail};
use crate::poll::{POLL_TEXT_MAX, PollItem, PollMark, PollResult, Pollable, rfc3339, rfc3339_nano, without_secrets};
use crate::reading::is_reading;
use crate::redact::{redact_text, render, render_value};
use crate::script::{InjectSpec, auth_secrets};
use crate::source::{Provider, Shared, Tool, Var, new_tool};
use crate::text::{cut_bytes, float_short, quote_escaped, value_text_of};
use crate::when::{parse_span, parse_when};

use reading::{GrafanaReading, prom_number};

pub(crate) struct Grafana {
    env: Env,
    now: Clock,
    http: Http,
    url: String,
    user: String,
    password: String,
    token: String,
}

pub(crate) fn new(env: Env, shared: Shared) -> Arc<dyn Provider> {
    Arc::new(Grafana {
        url: env.get("GRAFANA_URL"),
        user: env.get("GRAFANA_USER"),
        password: env.get("GRAFANA_PASSWORD"),
        token: env.get("GRAFANA_TOKEN"),
        env,
        now: shared.now,
        http: shared.http,
    })
}

/// The tool `name` of `me`, answering what `run` does with the agent's input.
fn tool<In, F, Fut>(me: &Arc<Grafana>, name: &str, description: &str, run: F) -> Tool
where
    In: DeserializeOwned + JsonSchema + Send + 'static,
    F: Fn(Arc<Grafana>, In) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<String>> + Send + 'static,
{
    let me = Arc::clone(me);
    new_tool(name, description, move |input: In| run(Arc::clone(&me), input))
}

impl Provider for Grafana {
    fn name(&self) -> String {
        "Grafana".to_owned()
    }

    fn missing(&self) -> Vec<String> {
        let mut out = missing(&self.env, &["GRAFANA_URL"]);
        if self.token.is_empty() {
            out.extend(missing(&self.env, &["GRAFANA_USER", "GRAFANA_PASSWORD"]));
        }
        out
    }

    fn vars(&self) -> Vec<Var> {
        vec![
            Var::plain("GRAFANA_URL", "the address of the Grafana server").address(),
            Var::plain(
                "GRAFANA_TOKEN",
                "a service-account token that may only read; with it no user and password are needed",
            )
            .secret()
            .optional(),
            Var::plain("GRAFANA_USER", "a Grafana user, when there is no token").optional(),
            Var::plain("GRAFANA_PASSWORD", "that user's password, when there is no token")
                .secret()
                .optional(),
        ]
    }

    fn tools(self: Arc<Self>) -> Vec<Tool> {
        vec![
            tool(
                &self,
                "grafana_datasources",
                "List the datasources Grafana reads (uid, type, name). Use the uid of a prometheus one as the datasource of grafana_promql.",
                |g, _: GrafanaNone| async move { g.datasources().await },
            ),
            tool(
                &self,
                "grafana_promql",
                "Run a PromQL query on a Prometheus-compatible datasource (VictoriaMetrics). With no start it is an instant query at `at` (default now); with a start it is a range query, and each series is summarised (points, first, last, min, max, average). Times are RFC3339 or relative to now such as -30m, -6h, -7d. Use an instant query to see what a number is now before it becomes a signal: when it answers one number, the answer ends with the query a sensor takes, and a measure of an objective reads that signal's `value`.",
                |g, input: PromqlInput| async move { g.promql(input).await },
            ),
            tool(
                &self,
                "grafana_alerts",
                "The Grafana alert rules with their state (firing, pending, inactive), health, labels, summary and the instances that are firing now. Filter by state or by a word of the rule's name. A sensor can read the rules whose name has a word: each instance that starts firing (Alerting, NoData or Error) becomes an event, and one that resolves or is still pending does not.",
                |g, input: AlertsInput| async move { g.alerts(input).await },
            ),
            tool(
                &self,
                "grafana_dashboards",
                "Search Grafana dashboards by title or tag. With a uid, give the dashboard's panels and the queries each runs, which show the metric names the owner already watches.",
                |g, input: DashboardsInput| async move { g.dashboards(input).await },
            ),
        ]
    }

    /// How a script reaches Grafana (form proxy): the same address and the same Authorization as the typed tools, and GET
    /// only, because a POST to Grafana can change it (a datasource query can run any statement its datasource accepts).
    fn inject(&self) -> Option<InjectSpec> {
        let auth = self.authorization();
        Some(InjectSpec {
            base: self.url.clone(),
            headers: BTreeMap::from([("Authorization".to_owned(), auth)]),
            secrets: self.secrets(),
            ..InjectSpec::default()
        })
    }

    fn poller(self: Arc<Self>) -> Option<Arc<dyn Pollable>> {
        Some(self)
    }
}

impl Grafana {
    /// The Authorization header Grafana is asked with: the service account's token, else the user's login.
    fn authorization(&self) -> String {
        if !self.token.is_empty() {
            return format!("Bearer {}", self.token);
        }
        let login = base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", self.user, self.password));
        format!("Basic {login}")
    }

    /// The values of the credential, in every spelling an answer may echo.
    fn secrets(&self) -> Vec<String> {
        auth_secrets(&self.authorization(), &[self.token.clone(), self.password.clone()])
    }

    fn api(&self) -> Api {
        Api {
            name: self.name(),
            base: self.url.clone(),
            headers: vec![("Authorization".to_owned(), self.authorization())],
            http: self.http.clone(),
        }
    }

    async fn datasources(&self) -> Result<String> {
        render(&self.list_datasources().await?)
    }

    async fn list_datasources(&self) -> Result<Vec<DatasourceRef>> {
        let raw = self.api().get("/api/datasources", &[]).await?;
        let list: Option<Vec<DatasourceRef>> =
            serde_json::from_slice(&raw).map_err(|e| err!("grafana: the datasources are not JSON: {e}"))?;
        Ok(list.unwrap_or_default())
    }

    async fn promql(&self, input: PromqlInput) -> Result<String> {
        if input.query.trim().is_empty() {
            return fail!("query is empty");
        }
        let uid = self.prometheus(&input.datasource).await?;
        let now = self.now.now();
        if input.start.is_empty() {
            let at = parse_when(&input.at, now).map_err(|e| e.context("at"))?;
            let raw = self.instant(&uid, &input.query, at).await?;
            let mut out = summarise_prom_result(&raw)?;
            if prom_number(&raw).is_ok() {
                let r = GrafanaReading {
                    datasource: uid,
                    query: input.query,
                };
                out.push_str(&format!("As a signal: {r}\n"));
            }
            return Ok(out);
        }
        let start = parse_when(&input.start, now).map_err(|e| e.context("start"))?;
        let end = parse_when(&input.end, now).map_err(|e| e.context("end"))?;
        if end <= start {
            return fail!("end is not after start");
        }
        let step = if input.step.is_empty() {
            None
        } else {
            match parse_span(&input.step) {
                Some(d) if d.is_positive() => Some(d),
                _ => return fail!("step {:?} is not a duration such as 30s or 5m", input.step),
            }
        };
        let q: Query = vec![
            ("query".to_owned(), input.query),
            ("start".to_owned(), start.as_second().to_string()),
            ("end".to_owned(), end.as_second().to_string()),
            ("step".to_owned(), auto_step(start, end, step).as_secs().to_string()),
        ];
        let raw = self.prom_get(&uid, "/api/v1/query_range", &q).await?;
        summarise_prom_result(&raw)
    }

    /// The uid of the datasource `wanted` names (a uid or a name); an empty one is the only prometheus datasource there is.
    async fn prometheus(&self, wanted: &str) -> Result<String> {
        let list = self.list_datasources().await?;
        let mut proms = Vec::new();
        for d in list {
            if !wanted.is_empty() && (d.uid == wanted || d.name == wanted) {
                if d.kind != "prometheus" {
                    return fail!(
                        "datasource {wanted:?} is {}, not prometheus: this tool runs PromQL",
                        d.kind
                    );
                }
                return Ok(d.uid);
            }
            if d.kind == "prometheus" {
                proms.push(d);
            }
        }
        if !wanted.is_empty() {
            return fail!("no datasource {wanted:?}: grafana_datasources lists them");
        }
        match proms.as_slice() {
            [one] => Ok(one.uid.clone()),
            [] => fail!("grafana has no prometheus datasource"),
            many => fail!(
                "grafana has {} prometheus datasources: name one as datasource",
                many.len()
            ),
        }
    }

    /// The answer of a PromQL instant query at `at` on the prometheus datasource `uid`: what grafana_promql shows and a
    /// reading reads its number from.
    async fn instant(&self, uid: &str, query: &str, at: Timestamp) -> Result<Vec<u8>> {
        let q: Query = vec![
            ("query".to_owned(), query.to_owned()),
            ("time".to_owned(), at.as_second().to_string()),
        ];
        self.prom_get(uid, "/api/v1/query", &q).await
    }

    /// A GET of the Prometheus HTTP API of the datasource `uid`, through Grafana's datasource proxy.
    async fn prom_get(&self, uid: &str, endpoint: &str, q: &[(String, String)]) -> Result<Vec<u8>> {
        let path = format!("/api/datasources/proxy/uid/{}{endpoint}", urlescape::path_segment(uid));
        self.api().get(&path, q).await
    }

    async fn alerts(&self, input: AlertsInput) -> Result<String> {
        let rules = self.rules_named(&input.name).await?;
        let want = input.state.trim().to_lowercase();
        let mut out: Vec<AlertRule> = rules
            .into_iter()
            .filter(|r| want.is_empty() || r.state == want)
            .collect();
        // Firing first: that is what an agent looks for.
        out.sort_by_key(|r| alert_rank(&r.state));
        if out.is_empty() {
            return Ok("No alert rule matches.".to_owned());
        }
        render(&out)
    }

    /// Grafana's alert rules whose name contains `word`, any case; an empty word is every rule. It is what grafana_alerts
    /// lists and an alert poll reads.
    async fn rules_named(&self, word: &str) -> Result<Vec<AlertRule>> {
        let raw = self.api().get("/api/prometheus/grafana/api/v1/rules", &[]).await?;
        let mut rules = alert_rules_of(&raw)?;
        let word = word.trim().to_lowercase();
        rules.retain(|r| r.name.to_lowercase().contains(&word));
        Ok(rules)
    }

    async fn dashboards(&self, input: DashboardsInput) -> Result<String> {
        if !input.uid.is_empty() {
            let raw = self
                .api()
                .get(
                    &format!("/api/dashboards/uid/{}", urlescape::path_segment(&input.uid)),
                    &[],
                )
                .await?;
            return dashboard_panels(&raw);
        }
        let mut q: Query = vec![
            ("type".to_owned(), "dash-db".to_owned()),
            ("limit".to_owned(), "100".to_owned()),
        ];
        if !input.query.is_empty() {
            q.push(("query".to_owned(), input.query));
        }
        if !input.tag.is_empty() {
            q.push(("tag".to_owned(), input.tag));
        }
        let raw = self.api().get("/api/search", &q).await?;
        let found: Option<Vec<Map<String, Value>>> =
            serde_json::from_slice(&raw).map_err(|e| err!("grafana: the search is not JSON: {e}"))?;
        let field = |d: &Map<String, Value>, k: &str| d.get(k).cloned().unwrap_or(Value::Null);
        let out: Vec<Value> = found
            .unwrap_or_default()
            .iter()
            .map(|d| {
                serde_json::json!({
                    "uid": field(d, "uid"),
                    "title": field(d, "title"),
                    "folder": field(d, "folderTitle"),
                    "tags": field(d, "tags"),
                })
            })
            .collect();
        render_value(&Value::Array(out))
    }

    // ---- the poll ----

    async fn poll_items(&self, query: &str, cursor: &str, limit: usize) -> Result<PollResult> {
        if is_reading(query) {
            return self.poll_reading(query, cursor).await;
        }
        let mut mark = if cursor.is_empty() {
            PollMark::default()
        } else {
            PollMark::parse(cursor)?
        };
        let firing = self.firing(query).await?;
        if cursor.is_empty() {
            for f in &firing {
                mark.take(f.at, &f.id);
            }
            return Ok(PollResult {
                items: Vec::new(),
                cursor: mark.to_string(),
            });
        }
        let mut items = Vec::new();
        for f in &firing {
            if mark.handed(f.at, &f.id) {
                continue;
            }
            items.push(self.poll_item(f)?);
            mark.take(f.at, &f.id);
            if items.len() == limit {
                break;
            }
        }
        Ok(PollResult {
            items,
            cursor: mark.to_string(),
        })
    }

    /// The instances of the rules named by `word` that fire now, oldest first.
    async fn firing(&self, word: &str) -> Result<Vec<Stamped>> {
        let rules = self.rules_named(word).await?;
        let mut out = Vec::new();
        for r in &rules {
            for instance in &r.instances {
                let state = instance
                    .state
                    .split_once(" (")
                    .map_or(instance.state.as_str(), |(s, _)| s);
                // Not firing, or without a time it fired at.
                let Ok(at) = instance.active_at.parse::<Timestamp>() else {
                    continue;
                };
                if !GRAFANA_FIRING.contains(&state) {
                    continue;
                }
                // A rule's uid names it; a Grafana that answers none is named by the folder and name it shows.
                let rule_id = if r.uid.is_empty() {
                    format!("{}/{}", r.folder, r.name)
                } else {
                    r.uid.clone()
                };
                out.push(Stamped {
                    instance: instance.clone(),
                    rule: r.clone(),
                    id: format!("{rule_id}/{}", labels_print(&instance.labels)),
                    at,
                });
            }
        }
        out.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.id.cmp(&b.id)));
        Ok(out)
    }

    fn poll_item(&self, f: &Stamped) -> Result<PollItem> {
        let text = redact_text(&f.rule.summary);
        let summary = cut_bytes(&text, POLL_TEXT_MAX);
        let stamp = rfc3339_nano(f.at);
        let url = if f.rule.uid.is_empty() {
            String::new()
        } else {
            format!(
                "{}/alerting/grafana/{}/view",
                self.url.trim_end_matches('/'),
                urlescape::path_segment(&f.rule.uid)
            )
        };
        let body = AlertBody {
            rule_uid: f.rule.uid.clone(),
            rule: f.rule.name.clone(),
            folder: f.rule.folder.clone(),
            state: f.instance.state.clone(),
            active_at: stamp.clone(),
            labels: f.instance.labels.clone(),
            value: f.instance.value.clone(),
            summary: summary.to_owned(),
            summary_cut: summary.len() < text.len(),
            url,
        };
        Ok(PollItem {
            id: f.id.clone(),
            version: format!("{}@{stamp}", f.instance.state),
            body: render(&body)?,
        })
    }
}

/// The alert instances that started firing since `cursor`, oldest first, or a reading (grafana/reading.rs) when the query
/// starts with `reading:`. Otherwise the query is a word of the rules' names, as grafana_alerts takes it (empty: every rule).
/// An item's id is the rule's uid and the fingerprint of the instance's labels, and its version `<state>@<when it entered
/// it>`, so an instance that fires, resolves and fires again is two events. One that resolves, is pending or recovers is no
/// item. An empty cursor answers no instance and marks the newest one firing now (none: the zero stamp, after which every
/// one that fires is new). Each poll is one GET of the rules (GET /api/prometheus/grafana/api/v1/rules, which lists each
/// rule's instances).
impl Pollable for Grafana {
    fn poll<'a>(&'a self, query: &'a str, cursor: &'a str, limit: usize) -> BoxFuture<'a, Result<PollResult>> {
        Box::pin(async move { without_secrets(self.poll_items(query, cursor, limit).await, &self.secrets()) })
    }
}

/// The states of an alert instance Grafana notifies of, as its Prometheus-compatible API spells them before a reason in
/// brackets ("Alerting (NoData)"): the others are Normal, Pending and Recovering.
const GRAFANA_FIRING: [&str; 3] = ["Alerting", "NoData", "Error"];

/// An alert instance that fires, with its rule, its id as an item names it and when it entered its state.
struct Stamped {
    instance: AlertInstance,
    rule: AlertRule,
    id: String,
    at: Timestamp,
}

/// The fingerprint of an instance's labels (FNV-1a, 64 bits): the same labels, in any order, are the
/// same instance of its rule.
fn labels_print(labels: &BTreeMap<String, String>) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    for (k, v) in labels {
        for b in k.bytes().chain([0]).chain(v.bytes()).chain([0]) {
            h ^= u64::from(b);
            h = h.wrapping_mul(PRIME);
        }
    }
    format!("{h:x}")
}

/// What an event of an alert instance carries: enough to decide and to open it, the summary cut at `POLL_TEXT_MAX`.
#[derive(Serialize)]
struct AlertBody {
    #[serde(skip_serializing_if = "String::is_empty")]
    rule_uid: String,
    rule: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    folder: String,
    state: String,
    /// When the instance entered its state.
    active_at: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    labels: BTreeMap<String, String>,
    /// The values of the rule's queries that made it fire, as Grafana writes them.
    #[serde(skip_serializing_if = "String::is_empty")]
    value: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    summary: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    summary_cut: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    url: String,
}

// ---- inputs ----

/// grafana_datasources takes nothing.
#[derive(Deserialize, JsonSchema)]
struct GrafanaNone {}

#[derive(Deserialize, JsonSchema)]
struct PromqlInput {
    /// the PromQL expression
    query: String,
    /// uid or name of a prometheus datasource; empty when Grafana has exactly one
    #[serde(default)]
    datasource: String,
    /// instant query: the time (RFC3339 or -30m); default now
    #[serde(default)]
    at: String,
    /// range query: where it begins (RFC3339 or -6h); setting it makes the query a range
    #[serde(default)]
    start: String,
    /// range query: where it ends; default now
    #[serde(default)]
    end: String,
    /// range query: resolution such as 1m; default chosen to give about 120 points
    #[serde(default)]
    step: String,
}

#[derive(Deserialize, JsonSchema)]
struct AlertsInput {
    /// only rules in this state: firing, pending, inactive
    #[serde(default)]
    state: String,
    /// only rules whose name contains this word, any case
    #[serde(default)]
    name: String,
}

#[derive(Deserialize, JsonSchema)]
struct DashboardsInput {
    /// words of a dashboard's title; empty lists them all
    #[serde(default)]
    query: String,
    /// only dashboards with this tag
    #[serde(default)]
    tag: String,
    /// a dashboard's uid, from the search: gives its panels and the queries they run
    #[serde(default)]
    uid: String,
}

// ---- answers ----

#[derive(Debug, Serialize, Deserialize)]
struct DatasourceRef {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    uid: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default, rename = "type")]
    kind: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    name: String,
}

/// How many series of one result are shown.
const MAX_SERIES: usize = 40;

/// The resolution of a range query: `step` when given (at least a second), else the one that makes about 120 points of the
/// range, at least 15s.
fn auto_step(start: Timestamp, end: Timestamp, step: Option<SignedDuration>) -> SignedDuration {
    if let Some(s) = step {
        return s.max(SignedDuration::from_secs(1));
    }
    let span = end.duration_since(start);
    let each = span
        .checked_div(120)
        .and_then(|d| d.round(SignedDurationRound::new().smallest(Unit::Second)).ok())
        .unwrap_or(SignedDuration::ZERO);
    each.max(SignedDuration::from_secs(15))
}

#[derive(Default, Deserialize)]
struct PromResponse {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    status: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default, rename = "errorType")]
    error_type: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    error: String,
    #[serde(default)]
    data: Option<PromData>,
}

#[derive(Default, Deserialize)]
struct PromData {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default, rename = "resultType")]
    result_type: String,
    #[serde(default)]
    result: Option<Vec<Value>>,
}

#[derive(Default, Deserialize)]
struct PromSeries {
    #[serde(default)]
    metric: Option<BTreeMap<String, String>>,
    #[serde(default)]
    value: Option<Vec<Value>>,
    #[serde(default)]
    values: Option<Vec<Vec<Value>>>,
}

fn prom_response(raw: &[u8]) -> Result<(PromResponse, PromData)> {
    let mut r: PromResponse =
        serde_json::from_slice(raw).map_err(|e| err!("grafana: the answer is not Prometheus JSON: {e}"))?;
    let data = r.data.take().unwrap_or_default();
    Ok((r, data))
}

/// A Prometheus answer laid out for an agent: one line per series. A range series is reduced to its count, first, last,
/// min, max and average, because its thousand points would fill the context and say less.
fn summarise_prom_result(raw: &[u8]) -> Result<String> {
    let (r, data) = prom_response(raw)?;
    if r.status != "success" {
        return fail!("prometheus {}: {}", r.error_type, r.error);
    }
    let result = data.result.unwrap_or_default();
    let mut b = format!("{}, {} series\n", data.result_type, result.len());
    for (i, item) in result.iter().enumerate() {
        if i == MAX_SERIES {
            b.push_str(&format!(
                "[{} more series not shown; aggregate with sum by (...) or add a label matcher]\n",
                result.len().saturating_sub(MAX_SERIES)
            ));
            break;
        }
        if data.result_type == "scalar" || data.result_type == "string" {
            b.push_str(&format!("{item}\n"));
            continue;
        }
        let s: PromSeries =
            serde_json::from_value(item.clone()).map_err(|e| err!("grafana: a series is not Prometheus JSON: {e}"))?;
        b.push_str(&labels_of(&s.metric.unwrap_or_default()));
        b.push(' ');
        let values = s.values.unwrap_or_default();
        match s.value.as_deref() {
            _ if !values.is_empty() => b.push_str(&range_summary(&values)),
            Some([at, v]) => b.push_str(&format!("= {} @ {}", value_text_of(v), when_of(at))),
            _ => {}
        }
        b.push('\n');
    }
    Ok(b)
}

/// Labels as `{k="v", ...}`, sorted by name.
pub(crate) fn labels_of(m: &BTreeMap<String, String>) -> String {
    if m.is_empty() {
        return "{}".to_owned();
    }
    let parts: Vec<String> = m.iter().map(|(k, v)| format!("{k}={}", quote_escaped(v))).collect();
    format!("{{{}}}", parts.join(", "))
}

/// A Prometheus timestamp (seconds since the epoch) as RFC3339; anything else as it is written.
fn when_of(ts: &Value) -> String {
    ts.as_f64()
        .and_then(|f| format!("{:.0}", f.trunc()).parse::<i64>().ok())
        .and_then(|s| Timestamp::from_second(s).ok())
        .map_or_else(|| value_text_of(ts), rfc3339)
}

/// A series summarised: how many numeric points, the first and last with their times, the least, the most and the mean.
pub(crate) fn range_summary(values: &[Vec<Value>]) -> String {
    let mut n: usize = 0;
    let mut count = 0.0_f64;
    let mut sum = 0.0_f64;
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut first: Option<(&Value, &Value)> = None;
    let mut last: Option<(&Value, &Value)> = None;
    for p in values {
        let [at, v] = p.as_slice() else {
            continue;
        };
        let Ok(x) = value_text_of(v).parse::<f64>() else {
            continue;
        };
        if x.is_nan() {
            continue;
        }
        if first.is_none() {
            first = Some((at, v));
        }
        last = Some((at, v));
        n = n.saturating_add(1);
        count += 1.0;
        sum += x;
        lo = lo.min(x);
        hi = hi.max(x);
    }
    let (Some((first_at, first_v)), Some((last_at, last_v))) = (first, last) else {
        return "no numeric points".to_owned();
    };
    format!(
        "{n} points  first {} @ {}  last {} @ {}  min {}  max {}  avg {}",
        value_text_of(first_v),
        when_of(first_at),
        value_text_of(last_v),
        when_of(last_at),
        float_short(lo),
        float_short(hi),
        float_short(sum / count)
    )
}

#[derive(Debug, Clone, Serialize)]
struct AlertRule {
    #[serde(skip_serializing_if = "String::is_empty")]
    uid: String,
    name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    folder: String,
    state: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    health: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    paused: bool,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    labels: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    summary: String,
    #[serde(rename = "lastEvaluation", skip_serializing_if = "String::is_empty")]
    last_evaluation: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    instances: Vec<AlertInstance>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AlertInstance {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    state: String,
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "crate::null_json::null_as_zero"
    )]
    labels: BTreeMap<String, String>,
    #[serde(
        deserialize_with = "crate::null_json::null_as_zero",
        default,
        rename = "activeAt",
        skip_serializing_if = "String::is_empty"
    )]
    active_at: String,
    #[serde(
        deserialize_with = "crate::null_json::null_as_zero",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    value: String,
}

fn alert_rank(state: &str) -> u8 {
    match state {
        "firing" => 0,
        "pending" => 1,
        _ => 2,
    }
}

#[derive(Default, Deserialize)]
struct RulesAnswer {
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    data: RulesData,
}

#[derive(Default, Deserialize)]
struct RulesData {
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    groups: Vec<RuleGroup>,
}

#[derive(Default, Deserialize)]
struct RuleGroup {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    file: String,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    rules: Vec<RawRule>,
}

#[derive(Default, Deserialize)]
struct RawRule {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    uid: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    name: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    state: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    health: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default, rename = "isPaused")]
    is_paused: bool,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    labels: BTreeMap<String, String>,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    annotations: BTreeMap<String, String>,
    #[serde(
        deserialize_with = "crate::null_json::null_as_zero",
        default,
        rename = "lastEvaluation"
    )]
    last_evaluation: String,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    alerts: Vec<AlertInstance>,
}

fn alert_rules_of(raw: &[u8]) -> Result<Vec<AlertRule>> {
    let r: RulesAnswer = serde_json::from_slice(raw).map_err(|e| err!("grafana: the rules are not JSON: {e}"))?;
    let mut out = Vec::new();
    for grp in r.data.groups {
        for x in grp.rules {
            out.push(AlertRule {
                uid: x.uid,
                name: x.name,
                folder: grp.file.clone(),
                state: x.state,
                health: x.health,
                paused: x.is_paused,
                labels: x.labels,
                summary: x.annotations.get("summary").cloned().unwrap_or_default(),
                last_evaluation: x.last_evaluation,
                instances: x.alerts,
            });
        }
    }
    Ok(out)
}

#[derive(Serialize)]
struct Panel {
    title: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    queries: Vec<String>,
}

/// A dashboard reduced to what an agent can use: each panel's title, type and the queries it runs.
fn dashboard_panels(raw: &[u8]) -> Result<String> {
    // Typed strictly: a title that is not text, or a top-level panel that is not an object, is not a dashboard.
    #[derive(Deserialize, Default)]
    struct Answer {
        #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
        dashboard: Dashboard,
    }
    #[derive(Deserialize, Default)]
    struct Dashboard {
        #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
        title: String,
        #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
        panels: Vec<Map<String, Value>>,
    }
    let d: Answer = serde_json::from_slice(raw).map_err(|e| err!("grafana: the dashboard is not JSON: {e}"))?;
    let top: Vec<Value> = d.dashboard.panels.into_iter().map(Value::Object).collect();
    let mut panels = Vec::new();
    walk_panels(&top, &mut panels);
    render(&serde_json::json!({"title": d.dashboard.title, "panels": panels}))
}

fn walk_panels(ps: &[Value], panels: &mut Vec<Panel>) {
    let text = |v: Option<&Value>| v.and_then(Value::as_str).unwrap_or_default().to_owned();
    for p in ps.iter().filter(|p| p.is_object()) {
        let mut x = Panel {
            title: text(p.get("title")),
            kind: text(p.get("type")),
            queries: Vec::new(),
        };
        for t in p.get("targets").and_then(Value::as_array).into_iter().flatten() {
            if let Some(q) = ["expr", "rawSql", "query", "queryText"]
                .iter()
                .map(|k| text(t.get(k)))
                .find(|s| !s.is_empty())
            {
                x.queries.push(q);
            }
        }
        let inner = p.get("panels").and_then(Value::as_array);
        if x.kind != "row" || !x.queries.is_empty() {
            panels.push(x);
        }
        if let Some(inner) = inner {
            walk_panels(inner, panels);
        }
    }
}
