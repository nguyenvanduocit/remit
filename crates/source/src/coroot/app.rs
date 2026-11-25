//! One application as Coroot shows it (its checks, traffic, instances, links and incidents), and its logs.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Coroot, CorootApp, Incident, incident_row, label, or, resolve};
use crate::error::{Result, fail};
use crate::redact::{mask_emails, render};
use crate::text::{float_short, limit, table};

#[derive(Debug, Clone, Default, Deserialize)]
struct Cell {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    value: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct Row {
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    cells: Vec<Cell>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct WidgetTable {
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    header: Vec<String>,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    rows: Vec<Row>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct Series {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    name: String,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    data: Vec<Option<f64>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct Chart {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    title: String,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    series: Vec<Series>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct Widget {
    #[serde(default)]
    table: Option<WidgetTable>,
    #[serde(default)]
    chart: Option<Chart>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct Check {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    title: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    status: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    message: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    threshold: f64,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    unit: String,
    #[serde(
        deserialize_with = "crate::null_json::null_as_zero",
        default,
        rename = "condition_format_template"
    )]
    template: String,
}

impl Check {
    /// The rule the check applies, with its threshold in it: "the successful request percentage below 99%".
    fn condition(&self) -> String {
        let unit = match self.unit.as_str() {
            "percent" => "%",
            "second" => "s",
            _ => "",
        };
        let text = self
            .template
            .replace("<threshold>", &format!("{}{unit}", float_short(self.threshold)));
        // Words, not signs: Render escapes < and > in JSON, which an agent reads as noise.
        words_for_signs(&text)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
struct Report {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    name: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    status: String,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    checks: Vec<Check>,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    widgets: Vec<Widget>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct Link {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    id: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    status: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    link_status: String,
    #[serde(
        deserialize_with = "crate::null_json::null_as_zero",
        default,
        rename = "link_status_reason"
    )]
    reason: String,
    #[serde(default, rename = "link_stats", deserialize_with = "crate::null_json::null_as_zero")]
    stats: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct AppDetail {
    #[serde(default)]
    data: AppData,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct AppData {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    app_map: AppMap,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    reports: Vec<Report>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct AppMap {
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    clients: Vec<Link>,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    dependencies: Vec<Link>,
}

#[derive(Debug, Serialize)]
struct Finding {
    report: String,
    check: String,
    status: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    message: String,
    condition: String,
}

#[derive(Debug, Default, Serialize)]
struct Traffic {
    requests_per_second: f64,
    errors_per_second: f64,
    error_percent: f64,
    latency_share: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Neighbour {
    application: String,
    status: String,
    link: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    reason: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stats: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
struct AppAnswer {
    application: String,
    id: String,
    status: String,
    period: String,
    slo: Vec<Finding>,
    #[serde(rename = "checks_not_ok", skip_serializing_if = "Vec::is_empty")]
    not_ok: Vec<Finding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    traffic: Option<Traffic>,
    #[serde(rename = "instances_not_up", skip_serializing_if = "Vec::is_empty")]
    not_up: Vec<String>,
    instances: usize,
    #[serde(rename = "links_not_ok", skip_serializing_if = "Vec::is_empty")]
    unhealthy: Vec<Neighbour>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    incidents: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    no_data: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    more_not_shown: String,
}

#[derive(Deserialize, JsonSchema)]
pub(super) struct AppInput {
    /// a name, namespace/name or id from coroot_applications
    application: String,
    /// the period to read, from this to now (RFC3339 or relative such as -6h, -2d); default -1h
    #[serde(default)]
    since: String,
}

#[derive(Deserialize, JsonSchema)]
pub(super) struct LogsInput {
    /// a name, namespace/name or id from coroot_applications
    application: String,
    /// the least severe to show: warning, error (default) or critical
    #[serde(default)]
    severity: String,
    /// the period to read, from this to now (RFC3339 or relative such as -6h); default -1h
    #[serde(default)]
    since: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct LogPattern {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    severity: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    sample: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    sum: i64,
}

#[derive(Default, Deserialize)]
struct LogsAnswer {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    data: LogsData,
}

#[derive(Default, Deserialize)]
struct LogsData {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    status: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    message: String,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    patterns: Vec<LogPattern>,
}

/// How many error patterns of one application are shown.
const MAX_LOG_PATTERNS: usize = 15;

const SEVERITIES: [&str; 3] = ["warning", "error", "critical"];

impl Coroot {
    pub(super) async fn application(&self, input: AppInput) -> Result<String> {
        let q = self.window(&input.since)?;
        let o = self.overview().await?;
        let app = resolve(&o.data.applications, &input.application)?;
        let d: AppDetail = self.get_json(&self.project_path(&["app", &app.id]), &q).await?;
        let incidents = self.list_incidents().await?;
        render(&answer(app, &d, &incidents, or(&input.since, "-1h")))
    }

    pub(super) async fn logs(&self, input: LogsInput) -> Result<String> {
        let Some(least) = SEVERITIES.iter().position(|s| *s == or(&input.severity, "error")) else {
            return fail!("severity {:?} is not warning, error or critical", input.severity);
        };
        let mut q = self.window(&input.since)?;
        let o = self.overview().await?;
        let app = resolve(&o.data.applications, &input.application)?;
        // The view is a setting of the page, not a filter on the data: patterns groups similar messages.
        q.push(("query".to_owned(), r#"{"view":"patterns"}"#.to_owned()));
        let mut r: LogsAnswer = self.get_json(&self.project_path(&["app", &app.id, "logs"]), &q).await?;
        if r.data.status != "ok" {
            return fail!(
                "coroot has no logs for {}: {} {}",
                label(&app.id),
                r.data.status,
                r.data.message
            );
        }
        r.data.patterns.sort_by_key(|p| std::cmp::Reverse(p.sum));
        let rows: Vec<Vec<String>> = r
            .data
            .patterns
            .iter()
            .filter(|p| {
                SEVERITIES
                    .iter()
                    .position(|s| *s == p.severity)
                    .is_some_and(|i| i >= least)
            })
            .map(|p| {
                vec![
                    p.severity.clone(),
                    p.sum.to_string(),
                    limit(&mask_emails(&p.sample), 400),
                ]
            })
            .collect();
        let cols = ["severity", "count", "sample"].map(str::to_owned);
        Ok(format!("{}: {}\n", label(&app.id), r.data.message) + &table(&cols, &rows, MAX_LOG_PATTERNS))
    }
}

/// Coroot's page of one application (some 300 KB of charts) reduced to what explains its status.
fn answer(app: &CorootApp, d: &AppDetail, incidents: &[Incident], since: &str) -> AppAnswer {
    let mut a = AppAnswer {
        application: label(&app.id),
        id: app.id.clone(),
        status: app.status.clone(),
        period: format!("since {since}"),
        ..AppAnswer::default()
    };
    for r in &d.data.reports {
        for k in &r.checks {
            let f = Finding {
                report: r.name.clone(),
                check: k.title.clone(),
                status: k.status.clone(),
                message: k.message.clone(),
                condition: k.condition(),
            };
            if r.name == "SLO" {
                a.slo.push(f);
            } else if k.status != "ok" && k.status != "unknown" {
                a.not_ok.push(f);
            }
        }
        if r.name == "SLO" {
            a.traffic = traffic_of(&r.widgets);
        } else if r.name == "Instances" {
            (a.instances, a.not_up) = instances_of(&r.widgets);
        } else if r.status == "unknown" {
            a.no_data.push(r.name.clone());
        }
    }
    let map = &d.data.app_map;
    for l in map.dependencies.iter().chain(&map.clients) {
        if !matches!(l.link_status.as_str(), "ok" | "unknown" | "") {
            a.unhealthy.push(Neighbour {
                application: label(&l.id),
                status: l.status.clone(),
                link: l.link_status.clone(),
                reason: l.reason.clone(),
                stats: l.stats.clone(),
            });
        }
    }
    const KEEP: usize = 10;
    if a.unhealthy.len() > KEEP {
        a.more_not_shown = format!("{} more links not ok not shown", a.unhealthy.len().saturating_sub(KEEP));
        a.unhealthy.truncate(KEEP);
    }
    for i in incidents {
        if i.application_id == app.id && (i.open() || a.incidents.len() < 3) {
            a.incidents.push(incident_row(i).join(" | "));
        }
    }
    a
}

/// The SLO report's chart of requests per second (a series per latency bucket, one for errors).
fn traffic_of(widgets: &[Widget]) -> Option<Traffic> {
    let chart = widgets
        .iter()
        .filter_map(|w| w.chart.as_ref())
        .find(|c| c.title.starts_with("Requests"))?;
    let mut t = Traffic::default();
    let mut means = BTreeMap::new();
    for s in &chart.series {
        if s.name == "errors" {
            t.errors_per_second = mean(&s.data);
            continue;
        }
        let m = mean(&s.data);
        means.insert(s.name.clone(), m);
        t.requests_per_second += m;
    }
    if t.requests_per_second > 0.0 {
        t.error_percent = 100.0 * t.errors_per_second / t.requests_per_second;
        for s in chart.series.iter().filter(|s| s.name != "errors") {
            let share = 100.0 * means.get(&s.name).copied().unwrap_or(0.0) / t.requests_per_second;
            if share >= 1.0 {
                t.latency_share.push(format!("{} {share:.0}%", bucket(&s.name)));
            }
        }
    }
    t.requests_per_second = round4(t.requests_per_second);
    t.errors_per_second = round4(t.errors_per_second);
    t.error_percent = round4(t.error_percent);
    Some(t)
}

/// `x` to four decimals: a rate of 0.000216 errors a second is read as 0.0002, not as a 17-digit float.
fn round4(x: f64) -> f64 {
    (x * 1e4).round() / 1e4
}

/// A latency bucket's name without the sign Render would escape: ">10 s" is "over 10 s".
fn bucket(name: &str) -> String {
    name.strip_prefix('>')
        .map_or_else(|| name.to_owned(), |rest| format!("over {rest}"))
}

fn mean(data: &[Option<f64>]) -> f64 {
    let (sum, n) = data.iter().flatten().fold((0.0, 0.0), |(s, n), v| (s + v, n + 1.0));
    if n == 0.0 { 0.0 } else { sum / n }
}

/// How many instances the Instances report lists, and those that are not up or have restarted (at most 10).
fn instances_of(widgets: &[Widget]) -> (usize, Vec<String>) {
    let Some(t) = widgets.iter().find_map(|w| w.table.as_ref()) else {
        return (0, Vec::new());
    };
    let status = t.header.iter().position(|h| h == "Status");
    let restarts = t.header.iter().position(|h| h == "Restarts");
    let mut not_up = Vec::new();
    for row in &t.rows {
        let Some(si) = status else {
            continue;
        };
        let at = |i: Option<usize>| i.and_then(|i| row.cells.get(i)).map_or("", |c| c.value.as_str());
        let (Some(first), Some(state)) = (row.cells.first(), row.cells.get(si)) else {
            continue;
        };
        if restarts.is_some_and(|ri| ri >= row.cells.len()) {
            continue;
        }
        let up = state.value.starts_with("up");
        if !up || !at(restarts).is_empty() {
            not_up.push(
                format!("{}: {} {} restarts", first.value, state.value, at(restarts))
                    .trim()
                    .to_owned(),
            );
        }
    }
    not_up.truncate(10);
    (t.rows.len(), not_up)
}

/// `text` with " < " as " below " and " > " as " above ", in one pass from the left: a replaced sign is not read again.
fn words_for_signs(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if let Some(after) = rest.strip_prefix(" < ") {
            out.push_str(" below ");
            rest = after;
        } else if let Some(after) = rest.strip_prefix(" > ") {
            out.push_str(" above ");
            rest = after;
        } else {
            out.push(c);
            rest = rest.get(c.len_utf8()..).unwrap_or_default();
        }
    }
    out
}
