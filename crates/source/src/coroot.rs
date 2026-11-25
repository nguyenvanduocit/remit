//! Coroot is the eBPF observability platform: its app map says which applications are unhealthy now, its SLO checks say why
//! (availability and latency error budgets, restarts, memory, DNS, network), and its incidents say when an SLO started to
//! burn. Credentials: COROOT_PUBLIC_URL, COROOT_PUBLIC_PROJECT_ID and the viewer's COROOT_PUBLIC_USER with
//! COROOT_PUBLIC_PASSWORD. Coroot signs a user in with a session cookie, which `Coroot::session` obtains on the first call
//! and again after a 401.
//!
//! The endpoints (all under /api/project/{project}) were confirmed live against Coroot 1.x: /overview/applications,
//! /incidents, /incident/{key}, /app/{id} (JSON when the request asks for it) and /app/{id}/logs.

mod app;

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use jiff::Timestamp;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::{Api, Http, Query, cause_chain};
use crate::env::{Clock, Env, missing};
use crate::error::{Result, err, fail};
use crate::limit::Limit;
use crate::poll::rfc3339;
use crate::redact::{redact_text, render};
use crate::source::{Provider, Shared, Tool, Var, new_tool};
use crate::text::{float_short, table, value_text_of};
use crate::when::parse_when;

use app::{AppInput, LogsInput};

/// The name of the cookie Coroot keeps a session in.
const COOKIE: &str = "coroot_session";
/// The applications and the incidents one call lists.
const APPLICATIONS: Limit = Limit::new(40, 150);
const INCIDENTS: Limit = Limit::new(20, 100);

pub(crate) struct Coroot {
    env: Env,
    now: Clock,
    url: String,
    project: String,
    user: String,
    password: String,
    http: Http,
    /// The session cookie in use, once signed in.
    cookie: tokio::sync::Mutex<Option<String>>,
}

pub(crate) fn new(env: Env, shared: Shared) -> Arc<dyn Provider> {
    Arc::new(Coroot {
        url: env.get("COROOT_PUBLIC_URL"),
        project: env.get("COROOT_PUBLIC_PROJECT_ID"),
        user: env.get("COROOT_PUBLIC_USER"),
        password: env.get("COROOT_PUBLIC_PASSWORD"),
        env,
        now: shared.now,
        http: shared.http,
        cookie: tokio::sync::Mutex::new(None),
    })
}

/// The tool `name` of `me`, answering what `run` does with the agent's input.
fn tool<In, F, Fut>(me: &Arc<Coroot>, name: &str, description: &str, run: F) -> Tool
where
    In: DeserializeOwned + JsonSchema + Send + 'static,
    F: Fn(Arc<Coroot>, In) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<String>> + Send + 'static,
{
    let me = Arc::clone(me);
    new_tool(name, description, move |input: In| run(Arc::clone(&me), input))
}

impl Provider for Coroot {
    fn name(&self) -> String {
        "Coroot".to_owned()
    }

    fn missing(&self) -> Vec<String> {
        missing(
            &self.env,
            &[
                "COROOT_PUBLIC_URL",
                "COROOT_PUBLIC_PROJECT_ID",
                "COROOT_PUBLIC_USER",
                "COROOT_PUBLIC_PASSWORD",
            ],
        )
    }

    fn vars(&self) -> Vec<Var> {
        vec![
            Var::plain("COROOT_PUBLIC_URL", "the address of the Coroot server").address(),
            Var::plain("COROOT_PUBLIC_PROJECT_ID", "the id of the Coroot project to read"),
            Var::plain("COROOT_PUBLIC_USER", "the Coroot user that reads"),
            Var::plain("COROOT_PUBLIC_PASSWORD", "that user's password").secret(),
        ]
    }

    fn tools(self: Arc<Self>) -> Vec<Tool> {
        vec![
            tool(
                &self,
                "coroot_applications",
                "What is unhealthy now in the cluster: the applications Coroot watches with their status (critical, warning, info, ok, unknown) and the signals that are not ok (latency, errors, upstreams, instances, restarts, cpu, memory, disk, network, dns, logs), plus the cluster's open incidents and alerts. By default only warning and critical applications; filter by status, category or a word of the name. Start here, then use coroot_application on one of them.",
                |c, input: ApplicationsInput| async move { c.applications(input).await },
            ),
            tool(
                &self,
                "coroot_incidents",
                "Coroot's SLO incidents: which application, which SLO (availability or latency) burned its error budget, when it opened and resolved, severity, impact and the summary. Open ones first. Filter by application, state (open, resolved, all) and since. Give a key from the list to get that incident's SLO objective, compliance and Coroot's root-cause analysis when it has one.",
                |c, input: IncidentsInput| async move { c.incidents(input).await },
            ),
            tool(
                &self,
                "coroot_application",
                "Why one application is or was unhealthy, from Coroot: its SLO checks (availability and latency objectives with status and the burn rate message), every other check that is not ok (restarts, OOM, memory, CPU, network, DNS, logs), traffic over the period (requests per second, error rate, how latency is spread), instances that are down or restarting, dependencies and clients whose link is not ok, and its recent incidents. The application is a name, namespace/name or id from coroot_applications.",
                |c, input: AppInput| async move { c.application(input).await },
            ),
            tool(
                &self,
                "coroot_logs",
                "The error patterns in an application's logs, from Coroot (it groups similar messages): severity, one sample message and how many times each appeared in the period, most frequent first. Use after coroot_application shows a Logs or errors problem, or to see what an application complains about. The application is a name, namespace/name or id from coroot_applications.",
                |c, input: LogsInput| async move { c.logs(input).await },
            ),
        ]
    }
}

// ---- the session ----------------------------------------------------------------------------------------------------
//
// Coroot signs a user in by POST /api/login (the email and the password, answered with a session cookie); that call
// creates a session and reads or changes no data, which is why this one POST is allowed in a read-only source. The cookie
// is obtained on the first request and renewed once when Coroot answers 401 (the session lasts days).

impl Coroot {
    /// The API of this connection, signed in with `cookie`.
    fn api(&self, cookie: &str) -> Api {
        Api {
            name: self.name(),
            base: self.url.clone(),
            headers: vec![("Cookie".to_owned(), format!("{COOKIE}={cookie}"))],
            http: self.http.clone(),
        }
    }

    /// The answer to GET `path`, signed in: once more with a new session when Coroot refuses the one in use.
    async fn get(&self, path: &str, query: &[(String, String)]) -> Result<Vec<u8>> {
        let cookie = self.session(None).await?;
        match self.api(&cookie).get(path, query).await {
            Err(e) if e.status().is_some_and(|s| s.code == 401) => {
                let fresh = self.session(Some(&cookie)).await?;
                self.api(&fresh).get(path, query).await
            }
            other => other,
        }
    }

    /// The cookie in use. `stale` is the cookie a request was refused with: when another request has already replaced it,
    /// the new one is returned instead of signing in a second time.
    async fn session(&self, stale: Option<&str>) -> Result<String> {
        let mut held = self.cookie.lock().await;
        if let Some(c) = held.as_deref()
            && Some(c) != stale
        {
            return Ok(c.to_owned());
        }
        let fresh = self.login().await?;
        *held = Some(fresh.clone());
        Ok(fresh)
    }

    async fn login(&self) -> Result<String> {
        let name = self.name();
        let target = self.api("").target("/api/login", &[])?;
        let body = serde_json::to_vec(&serde_json::json!({"email": self.user, "password": self.password}))
            .map_err(|e| err!("{name}: {e}"))?;
        let res = self
            .http
            .client()
            .map_err(|e| e.context(&name))?
            .post(target)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| err!("{name}: sign-in: {}", redact_text(&cause_chain(&e.without_url()))))?;
        let status = res.status();
        if !status.is_success() {
            let text = format!("{} {}", status.as_u16(), status.canonical_reason().unwrap_or_default());
            return fail!(
                "{name}: sign-in refused ({}): check COROOT_PUBLIC_USER and COROOT_PUBLIC_PASSWORD",
                text.trim()
            );
        }
        for header in res.headers().get_all(reqwest::header::SET_COOKIE) {
            let pair = header
                .to_str()
                .unwrap_or_default()
                .split(';')
                .next()
                .unwrap_or_default();
            if let Some((n, v)) = pair.split_once('=')
                && n.trim() == COOKIE
            {
                // A quoted value is read without its quotes.
                let v = v.trim();
                let v = v.strip_prefix('"').and_then(|u| u.strip_suffix('"')).unwrap_or(v);
                return Ok(v.to_owned());
            }
        }
        fail!("{name}: sign-in answered without a {COOKIE} cookie")
    }

    /// The path of a resource under the configured project; the parts are escaped, never taken raw from an agent.
    fn project_path(&self, parts: &[&str]) -> String {
        let mut path = format!("/api/project/{}", urlescape::path_segment(&self.project));
        for p in parts {
            path.push('/');
            path.push_str(&urlescape::path_segment(p));
        }
        path
    }

    async fn get_json<T: DeserializeOwned>(&self, path: &str, query: &[(String, String)]) -> Result<T> {
        let raw = self.get(path, query).await?;
        serde_json::from_slice(&raw).map_err(|e| err!("coroot: the answer to {path} is not the JSON expected: {e}"))
    }

    /// The period an agent's `since` names, as Coroot's from and to query (milliseconds since the epoch).
    fn window(&self, since: &str) -> Result<Query> {
        let now = self.now.now();
        let from = parse_when(or(since, "-1h"), now).map_err(|e| e.context("since"))?;
        if from >= now {
            return fail!("since is not in the past");
        }
        Ok(vec![
            ("from".to_owned(), from.as_millisecond().to_string()),
            ("to".to_owned(), now.as_millisecond().to_string()),
        ])
    }

    async fn overview(&self) -> Result<Overview> {
        self.get_json(&self.project_path(&["overview", "applications"]), &[])
            .await
    }

    async fn applications(&self, input: ApplicationsInput) -> Result<String> {
        let status = or(&input.status, "problems");
        if !APP_STATUSES.contains(&status) {
            return fail!("status {:?} is not one of: {}", input.status, APP_STATUSES.join(", "));
        }
        let o = self.overview().await?;
        let mut rows: Vec<&CorootApp> = o
            .data
            .applications
            .iter()
            .filter(|a| wanted(a, status, &input.category, &input.query))
            .collect();
        rows.sort_by(|a, b| {
            status_rank(&a.status)
                .cmp(&status_rank(&b.status))
                .then_with(|| a.id.cmp(&b.id))
        });
        let cells: Vec<Vec<String>> = rows
            .iter()
            .map(|a| {
                let (_, kind, _) = split_id(&a.id);
                vec![
                    label(&a.id),
                    kind.to_owned(),
                    a.category.clone(),
                    a.status.clone(),
                    a.not_ok(),
                ]
            })
            .collect();
        let limit = APPLICATIONS.of("limit", input.limit)?;
        let cols = ["application", "kind", "category", "status", "signals not ok"].map(str::to_owned);
        Ok(format!(
            "{}\nShowing {} with status {status}:\n{}",
            o.header(),
            rows.len(),
            table(&cols, &cells, limit)
        ))
    }

    // ---- the incidents ----

    async fn list_incidents(&self) -> Result<Vec<Incident>> {
        #[derive(Deserialize)]
        struct Answer {
            #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
            data: Vec<Incident>,
        }
        let q: Query = vec![("limit".to_owned(), INCIDENT_FEED.to_string())];
        let mut r: Answer = self.get_json(&self.project_path(&["incidents"]), &q).await?;
        r.data
            .sort_by(|a, b| b.open().cmp(&a.open()).then_with(|| b.opened_at.cmp(&a.opened_at)));
        Ok(r.data)
    }

    async fn incidents(&self, input: IncidentsInput) -> Result<String> {
        let state = or(&input.state, "all");
        if !["open", "resolved", "all"].contains(&state) {
            return fail!("state {:?} is not open, resolved or all", input.state);
        }
        let since = parse_when(or(&input.since, "-7d"), self.now.now()).map_err(|e| e.context("since"))?;
        let all = self.list_incidents().await?;
        if !input.key.is_empty() {
            return self.incident_detail(&all, &input.key).await;
        }
        let application = input.application.to_lowercase();
        let rows: Vec<Vec<String>> = all
            .iter()
            .filter(|i| {
                !((state == "open" && !i.open())
                    || (state == "resolved" && i.open())
                    || i128::from(i.opened_at)
                        .checked_mul(1_000_000)
                        .is_none_or(|ns| ns < since.as_nanosecond())
                    || !i.application_id.to_lowercase().contains(&application))
            })
            .map(incident_row)
            .collect();
        let note = if all.len() == INCIDENT_FEED {
            format!("\n[Coroot was asked for its newest {INCIDENT_FEED} incidents; older ones are not listed]")
        } else {
            String::new()
        };
        let limit = INCIDENTS.of("limit", input.limit)?;
        let cols = ["key", "application", "severity", "state", "opened", "slo", "summary"].map(str::to_owned);
        Ok(table(&cols, &rows, limit) + &note)
    }

    async fn incident_detail(&self, all: &[Incident], key: &str) -> Result<String> {
        #[derive(Deserialize)]
        struct Answer {
            #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
            data: Incident,
        }
        let Some(found) = all.iter().find(|i| i.key == key) else {
            return fail!(
                "no incident with key {key:?} among Coroot's newest {INCIDENT_FEED}: coroot_incidents lists them"
            );
        };
        let r: Answer = self
            .get_json(&self.project_path(&["incident", &found.key]), &[])
            .await?;
        let d = r.data;
        let (root_cause, immediate_fixes) = if d.rca.status == "AI disabled" {
            (String::new(), String::new())
        } else {
            (
                or(&d.rca.root_cause, &d.rca.summary).to_owned(),
                d.rca.immediate_fix.clone(),
            )
        };
        render(&IncidentDetail {
            key: d.key.clone(),
            application: label(&d.application_id),
            severity: d.severity.clone(),
            state: d.state(),
            opened: time_of(d.opened_at),
            summary: d.short_description.clone(),
            slo: d.slo(),
            availability_slo: d.availability_slo.clone(),
            latency_slo: d.latency_slo.clone(),
            root_cause,
            immediate_fixes,
        })
    }
}

// ---- time and names -------------------------------------------------------------------------------------------------

/// `s`, or `fallback` when it is blank.
fn or<'a>(s: &'a str, fallback: &'a str) -> &'a str {
    if s.trim().is_empty() { fallback } else { s }
}

fn time_of(ms: i64) -> String {
    Timestamp::from_millisecond(ms).map_or_else(|_| ms.to_string(), rfc3339)
}

/// Whether `a` and `b` are the same text under simple Unicode case folding.
fn equal_fold(a: &str, b: &str) -> bool {
    a.chars()
        .flat_map(char::to_lowercase)
        .eq(b.chars().flat_map(char::to_lowercase))
}

/// A duration in days, hours and minutes: what an agent compares with "since".
fn span(ms: i64) -> String {
    const MINUTE: i64 = 60_000;
    const HOUR: i64 = 60 * MINUTE;
    let div = |a: i64, b: i64| a.checked_div(b).unwrap_or(0);
    let rem = |a: i64, b: i64| a.checked_rem(b).unwrap_or(0);
    let hours = div(ms, HOUR);
    if ms < MINUTE {
        return "<1m".to_owned();
    }
    if ms < HOUR {
        return format!("{}m", div(ms, MINUTE));
    }
    if ms < 24 * HOUR {
        return units(hours, "h", rem(div(ms, MINUTE), 60), "m");
    }
    units(div(hours, 24), "d", rem(hours, 24), "h")
}

/// "3h20m", and "3h" when the second unit is zero.
fn units(big: i64, big_unit: &str, small: i64, small_unit: &str) -> String {
    if small == 0 {
        return format!("{big}{big_unit}");
    }
    format!("{big}{big_unit}{small}{small_unit}")
}

/// The parts of a Coroot application id, project:namespace:kind:name. The name may hold colons (external services).
fn split_id(id: &str) -> (&str, &str, &str) {
    let parts: Vec<&str> = id.splitn(4, ':').collect();
    match parts.as_slice() {
        [_, ns, kind, name] => (ns, kind, name),
        _ => ("", "", id),
    }
}

/// How a tool names an application: namespace/name.
fn label(id: &str) -> String {
    let (ns, _, name) = split_id(id);
    if ns.is_empty() {
        name.to_owned()
    } else {
        format!("{ns}/{name}")
    }
}

// ---- the applications -----------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
struct Signal {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    status: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    value: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct CorootApp {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    id: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    category: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    status: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    errors: Signal,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    latency: Signal,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    upstreams: Signal,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    instances: Signal,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    restarts: Signal,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    cpu: Signal,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    memory: Signal,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    disk_io_load: Signal,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    disk_usage: Signal,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    network: Signal,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    dns: Signal,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    logs: Signal,
}

impl CorootApp {
    /// The signals of the application that are not ok, as "latency critical 10s".
    fn not_ok(&self) -> String {
        let named = [
            ("errors", &self.errors),
            ("latency", &self.latency),
            ("upstreams", &self.upstreams),
            ("instances", &self.instances),
            ("restarts", &self.restarts),
            ("cpu", &self.cpu),
            ("memory", &self.memory),
            ("disk io", &self.disk_io_load),
            ("disk usage", &self.disk_usage),
            ("network", &self.network),
            ("dns", &self.dns),
            ("logs", &self.logs),
        ];
        named
            .iter()
            .filter(|(_, s)| !matches!(s.status.as_str(), "ok" | "unknown" | ""))
            .map(|(name, s)| format!("{name} {} {}", s.status, s.value).trim().to_owned())
            .collect::<Vec<_>>()
            .join("; ")
    }
}

const STATUS_ORDER: [&str; 5] = ["critical", "warning", "info", "ok", "unknown"];

/// Orders statuses from the one that needs attention first.
fn status_rank(status: &str) -> usize {
    STATUS_ORDER
        .iter()
        .position(|s| *s == status)
        .unwrap_or(STATUS_ORDER.len())
}

#[derive(Default, Deserialize)]
struct Overview {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    context: OverviewContext,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    data: OverviewData,
}

#[derive(Default, Deserialize)]
struct OverviewContext {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    status: ClusterStatus,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    incidents: OpenIncidents,
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    alerts: BTreeMap<String, Value>,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    fluxcd: Issues,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    argocd: Issues,
}

#[derive(Default, Deserialize)]
struct ClusterStatus {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    status: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    error: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    prometheus: StoreStatus,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    node_agent: NodeAgent,
}

#[derive(Default, Deserialize)]
struct StoreStatus {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    status: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    message: String,
}

#[derive(Default, Deserialize)]
struct NodeAgent {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    nodes: i64,
}

#[derive(Default, Deserialize)]
struct OpenIncidents {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    application: i64,
}

#[derive(Default, Deserialize)]
struct Issues {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    issues: i64,
}

#[derive(Default, Deserialize)]
struct OverviewData {
    #[serde(default, deserialize_with = "crate::null_json::null_as_zero")]
    applications: Vec<CorootApp>,
}

impl Overview {
    /// The cluster in two lines: Coroot's own verdict, then how many applications are in each status.
    fn header(&self) -> String {
        let c = &self.context;
        let s = &c.status;
        let mut b = format!(
            "Coroot: status {}, {} nodes, {} open application incidents",
            or(&s.status, "unknown"),
            s.node_agent.nodes,
            c.incidents.application
        );
        for (k, v) in &c.alerts {
            b.push_str(&format!(", {} {k} alerts", value_text_of(v)));
        }
        b.push_str(&format!(
            ", flux issues {}, argocd issues {}",
            c.fluxcd.issues, c.argocd.issues
        ));
        if !s.error.is_empty() {
            b.push_str(&format!("\nCoroot reports an error: {}", s.error));
        }
        if !s.prometheus.status.is_empty() && s.prometheus.status != "ok" {
            b.push_str(&format!(
                "\nCoroot's metrics store is {}: {}",
                s.prometheus.status, s.prometheus.message
            ));
        }
        let apps = &self.data.applications;
        let parts: Vec<String> = STATUS_ORDER
            .iter()
            .filter_map(|st| {
                let n = apps.iter().filter(|a| a.status == *st).count();
                (n > 0).then(|| format!("{st} {n}"))
            })
            .collect();
        b.push_str(&format!("\n{} applications: {}", apps.len(), parts.join(", ")));
        b
    }
}

#[derive(Deserialize, JsonSchema)]
struct ApplicationsInput {
    /// problems (warning and critical; the default), critical, warning, info, ok, unknown or all
    #[serde(default)]
    status: String,
    /// only this category, for example application, control-plane or monitoring
    #[serde(default)]
    category: String,
    /// only applications whose id contains this text (a name or a namespace)
    #[serde(default)]
    query: String,
    /// most applications shown; default 40, at most 150
    #[serde(default)]
    limit: i64,
}

const APP_STATUSES: [&str; 7] = ["problems", "critical", "warning", "info", "ok", "unknown", "all"];

fn wanted(a: &CorootApp, status: &str, category: &str, query: &str) -> bool {
    let by_status = match status {
        "all" => true,
        "problems" => a.status == "warning" || a.status == "critical",
        other => a.status == other,
    };
    by_status
        && (category.is_empty() || equal_fold(&a.category, category))
        && (query.is_empty() || a.id.to_lowercase().contains(&query.to_lowercase()))
}

/// The one application `wanted` names: an id, namespace/name, or a name; or why it cannot be told.
fn resolve<'a>(apps: &'a [CorootApp], wanted: &str) -> Result<&'a CorootApp> {
    let wanted = wanted.trim();
    if wanted.is_empty() {
        return fail!("application is empty: name one from coroot_applications");
    }
    let lower = wanted.to_lowercase();
    let matches: [&dyn Fn(&CorootApp) -> bool; 4] = [
        &|a| a.id == wanted,
        &|a| equal_fold(&label(&a.id), wanted),
        &|a| equal_fold(split_id(&a.id).2, wanted),
        &|a| a.id.to_lowercase().contains(&lower),
    ];
    for m in matches {
        let found: Vec<&CorootApp> = apps.iter().filter(|a| m(a)).collect();
        match found.as_slice() {
            [] => {}
            [one] => return Ok(one),
            many => {
                let names: Vec<String> = many
                    .iter()
                    .take(8)
                    .map(|a| format!("{} ({})", label(&a.id), split_id(&a.id).1))
                    .collect();
                return fail!(
                    "{wanted:?} matches {} applications ({}): give namespace/name",
                    many.len(),
                    names.join(", ")
                );
            }
        }
    }
    fail!("no application {wanted:?}: coroot_applications lists them")
}

// ---- the incidents --------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
struct Burn {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    long_window: i64,
    #[serde(
        deserialize_with = "crate::null_json::null_as_zero",
        default,
        rename = "long_window_burn_rate"
    )]
    long_burn_rate: f64,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    severity: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    threshold: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct Impact {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    percentage: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct Details {
    #[serde(
        default,
        rename = "availability_burn_rates",
        deserialize_with = "crate::null_json::null_as_zero"
    )]
    availability_burn: Vec<Burn>,
    #[serde(
        default,
        rename = "latency_burn_rates",
        deserialize_with = "crate::null_json::null_as_zero"
    )]
    latency_burn: Vec<Burn>,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    availability_impact: Impact,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    latency_impact: Impact,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct Rca {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    status: String,
    #[serde(
        deserialize_with = "crate::null_json::null_as_zero",
        default,
        rename = "short_summary"
    )]
    summary: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    root_cause: String,
    #[serde(
        deserialize_with = "crate::null_json::null_as_zero",
        default,
        rename = "immediate_fixes"
    )]
    immediate_fix: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct Incident {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    key: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    application_id: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    opened_at: i64,
    #[serde(default)]
    resolved_at: Option<i64>,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    severity: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    short_description: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    duration: i64,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    details: Details,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    rca: Rca,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    availability_slo: Slo,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    latency_slo: Slo,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Slo {
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    objective: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    compliance: String,
    #[serde(deserialize_with = "crate::null_json::null_as_zero", default)]
    violated: bool,
}

impl Incident {
    const fn open(&self) -> bool {
        self.resolved_at.is_none()
    }

    /// Which objective burned and how: the impact of each and the windows whose burn rate went over their limit.
    fn slo(&self) -> String {
        let d = &self.details;
        let kinds = [
            ("availability", &d.availability_impact, &d.availability_burn),
            ("latency", &d.latency_impact, &d.latency_burn),
        ];
        let mut parts = Vec::new();
        for (name, impact, burn) in kinds {
            if impact.percentage <= 0.0 && !burn.iter().any(|b| b.severity != "ok") {
                continue;
            }
            let mut s = format!("{name} impact {:.1}%", impact.percentage);
            for b in burn.iter().filter(|b| b.severity != "ok") {
                s.push_str(&format!(
                    ", {} burn {:.1}x (limit {}x)",
                    span(b.long_window),
                    b.long_burn_rate,
                    float_short(b.threshold)
                ));
            }
            parts.push(s);
        }
        parts.join("; ")
    }

    fn state(&self) -> String {
        if self.open() {
            return format!("open {}", span(self.duration));
        }
        format!("resolved after {}", span(self.duration))
    }
}

/// How many of Coroot's newest incidents are read: all of them are older ones that no longer matter.
const INCIDENT_FEED: usize = 200;

#[derive(Deserialize, JsonSchema)]
struct IncidentsInput {
    /// only incidents of applications whose id contains this text (a name or namespace/name)
    #[serde(default)]
    application: String,
    /// open, resolved or all (default)
    #[serde(default)]
    state: String,
    /// only incidents opened after this (RFC3339 or relative such as -24h, -7d); default -7d
    #[serde(default)]
    since: String,
    /// the key of an incident from the list: gives its SLO objective, compliance and root-cause analysis
    #[serde(default)]
    key: String,
    /// most incidents shown; default 20, at most 100
    #[serde(default)]
    limit: i64,
}

fn incident_row(i: &Incident) -> Vec<String> {
    vec![
        i.key.clone(),
        label(&i.application_id),
        i.severity.clone(),
        i.state(),
        time_of(i.opened_at),
        i.slo(),
        i.short_description.clone(),
    ]
}

#[derive(Serialize)]
struct IncidentDetail {
    key: String,
    application: String,
    severity: String,
    state: String,
    opened: String,
    summary: String,
    slo: String,
    availability_slo: Slo,
    latency_slo: Slo,
    #[serde(skip_serializing_if = "String::is_empty")]
    root_cause: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    immediate_fixes: String,
}
