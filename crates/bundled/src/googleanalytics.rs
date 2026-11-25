//! Google Analytics is the web and app analytics of the owner's Google Analytics 4 property: how many people came, from where, on
//! which pages and devices, what they did, now and over any days. It reads the Data API v1beta
//! (https://developers.google.com/analytics/devguides/reporting/data/v1), whose runReport, runRealtimeReport and getMetadata only
//! read. It signs in as a service account (https://developers.google.com/identity/protocols/oauth2/service-account): the key the
//! owner pastes signs a JWT for the read-only scope, Google answers an access token for an hour, and the key itself never leaves
//! the plugin. Credentials: GOOGLE_ANALYTICS_SERVICE_ACCOUNT_KEY (the JSON key of a service account the owner made a Viewer of
//! the property) and GOOGLE_ANALYTICS_PROPERTY_ID (ADR-0066).

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::LazyLock;

use jiff::{SignedDuration, Timestamp};
use plugin_sdk::{Call, Definition, Effect, Error, Poll, Tool, Var};
use regex::Regex;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use source::{Api, CAP_CUSTOMERS, Http, Limit, Shared, limit, mask_emails, null_as_zero, redact_text, table};

mod reading;

const NAME: &str = "Google Analytics";
const DATA_URL: &str = "https://analyticsdata.googleapis.com";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const SCOPE: &str = "https://www.googleapis.com/auth/analytics.readonly";
const KEY_VAR: &str = "GOOGLE_ANALYTICS_SERVICE_ACCOUNT_KEY";
const PROPERTY_VAR: &str = "GOOGLE_ANALYTICS_PROPERTY_ID";
/// What one report of the Data API takes.
const MAX_METRICS: usize = 10;
const MAX_DIMENSIONS: usize = 9;
/// The most rows a report shows, and the rows when none is asked.
const ROWS: Limit = Limit::new(25, 100);
/// How many metrics or dimensions metadata lists.
const MAX_LISTED: usize = 150;
/// How far back a realtime report of a standard property reads (startMinutesAgo is at most 29).
const REALTIME_MINUTES: i64 = 30;
/// The most of Google's answer to a sign-in that is read.
const TOKEN_ANSWER_MAX: usize = 64 << 10;

/// A metric or dimension API name, custom ones included (customEvent:plan).
#[expect(clippy::expect_used, reason = "the pattern is a constant a test compiles")]
static FIELD_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z][A-Za-z0-9_:]{0,99}$").expect("the field name shape compiles"));

/// A day as the Data API takes it, resolved in the property's own time zone.
#[expect(clippy::expect_used, reason = "the pattern is a constant a test compiles")]
static DAY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{4}-\d{2}-\d{2}|\d{1,4}daysAgo|yesterday|today)$").expect("the day shape compiles")
});

/// One call's view of the property: the connection's key and property, the Data API's address and the time the call began.
#[derive(Clone)]
struct GoogleAnalytics {
    now: Timestamp,
    http: Http,
    data_url: String,
    key: String,
    property: String,
}

impl GoogleAnalytics {
    fn new(call: &Call, shared: &Shared, data_url: &str) -> GoogleAnalytics {
        GoogleAnalytics {
            now: shared.now.now(),
            http: shared.http.clone(),
            data_url: data_url.to_owned(),
            key: call.var(KEY_VAR).to_owned(),
            property: call.var(PROPERTY_VAR).to_owned(),
        }
    }
}

/// The plugin on the clock and client of `shared`: `plugin_sdk` calls it with the credentials of the connection a tool was called through.
pub(crate) fn plugin(shared: Shared) -> Definition {
    plugin_at(shared, DATA_URL)
}

fn plugin_at(shared: Shared, data_url: &str) -> Definition {
    let (polled, poll_url) = (shared.clone(), data_url.to_owned());
    Definition {
        label: NAME.into(),
        capability: CAP_CUSTOMERS.into(),
        vars: vec![
            Var {
                name: KEY_VAR.into(),
                what: "the JSON key of a Google Cloud service account (IAM, Service accounts, Keys, Add key, JSON), pasted whole; add the account's email as a Viewer of the property (Admin, Property access management)".into(),
                secret: true,
                ..Default::default()
            },
            Var {
                name: PROPERTY_VAR.into(),
                what: "the number of the Google Analytics 4 property (Admin, Property details, Property ID)".into(),
                ..Default::default()
            },
        ],
        tools: vec![
            tool(&shared, data_url, "report", "Metrics of the property over a range of days, optionally split by up to 9 dimensions and kept to exact dimension values: active users, sessions, page views, events, key events and revenue by date, country, page, source, device or event name. Answers 'how many came', 'from where', 'which pages' and 'did it drop', with the total of each metric and Google's notes on thresholding and sampling. googleanalytics_metadata names every metric and dimension.", GoogleAnalytics::report),
            tool(&shared, data_url, "realtime", "Metrics of the property over the last 1 to 30 minutes, optionally split by dimensions such as country, deviceCategory, unifiedScreenName or eventName: who is on the site or app now, and whether a release or a campaign moved it.", GoogleAnalytics::realtime),
            tool(&shared, data_url, "metadata", "The metrics and dimensions the property's reports take, with their names in Google Analytics, categories and types, the property's custom ones flagged, and the ones the service account may not read. Start here to learn the exact API names the other googleanalytics tools take.", GoogleAnalytics::metadata),
            tool(&shared, data_url, "reading", "One number of the property as a sensor reads it, to try before it becomes a signal: a metric over the last days (report) or minutes (realtime), kept to exact dimension values. Answers the reading as its event carries it (value null with no_data when Google has no number) and ends with the query a sensor of the googleanalytics source takes.", GoogleAnalytics::reading),
        ],
        poll: Some(Poll::new(move |call, q| {
            let ga = GoogleAnalytics::new(&call, &polled, &poll_url);
            async move { ga.poll(&q.query, &q.cursor).await }
        })),
        ..Default::default()
    }
}

/// A read tool whose handler is a method of `GoogleAnalytics`, built for each call from the connection's variables.
fn tool<In, F, Fut>(shared: &Shared, data_url: &str, name: &str, description: &str, run: F) -> Tool
where
    In: DeserializeOwned + JsonSchema + Send + 'static,
    F: Fn(GoogleAnalytics, In) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<String, Error>> + Send + 'static,
{
    let (shared, data_url) = (shared.clone(), data_url.to_owned());
    Tool::new(name, description, Effect::Read, move |call: Call, input: In| {
        run(GoogleAnalytics::new(&call, &shared, &data_url), input)
    })
}

// --- signing in ---

/// What the plugin reads of a service account's JSON key.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ServiceAccount {
    #[serde(rename = "type", deserialize_with = "null_as_zero")]
    kind: String,
    #[serde(deserialize_with = "null_as_zero")]
    client_email: String,
    #[serde(deserialize_with = "null_as_zero")]
    private_key: String,
    #[serde(deserialize_with = "null_as_zero")]
    private_key_id: String,
    #[serde(deserialize_with = "null_as_zero")]
    token_uri: String,
}

/// The property's number, from GOOGLE_ANALYTICS_PROPERTY_ID as the owner typed it (`123` or `properties/123`).
fn property(v: &str) -> Result<String, String> {
    let trimmed = v.trim();
    let id = trimmed.strip_prefix("properties/").unwrap_or(trimmed);
    if id.is_empty() || id.len() > 20 || !id.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!(
            "{PROPERTY_VAR} {:?} is not the number of a property: it is under Admin, Property details, Property ID",
            limit(v, 40)
        ));
    }
    Ok(id.to_owned())
}

/// The service account of the key, or why the key is not one. No error repeats any of the key.
fn account(key: &str) -> Result<ServiceAccount, String> {
    if key.trim().is_empty() {
        return Err(format!(
            "{KEY_VAR} is not set: the owner pastes the JSON key of a service account that is a Viewer of the property"
        ));
    }
    let not_a_key = || {
        format!(
            "{KEY_VAR} is not the JSON key of a service account: it is the whole file Google gives under IAM, Service accounts, Keys, Add key, JSON"
        )
    };
    let mut a: ServiceAccount = serde_json::from_str(key).map_err(|_| not_a_key())?;
    if a.kind != "service_account" || a.client_email.is_empty() || a.private_key.is_empty() {
        return Err(not_a_key());
    }
    if a.token_uri.is_empty() {
        TOKEN_URL.clone_into(&mut a.token_uri);
    }
    let address = reqwest::Url::parse(&a.token_uri)
        .ok()
        .is_some_and(|u| matches!(u.scheme(), "https" | "http") && u.host_str().is_some_and(|h| !h.is_empty()));
    if !address {
        return Err(format!(
            "{KEY_VAR} has a token_uri that is not an address: paste the key as Google gave it"
        ));
    }
    Ok(a)
}

/// The claims of the JWT a service account signs to sign in: issued ten seconds ago, for an hour.
#[derive(Serialize)]
struct Claims<'a> {
    iss: &'a str,
    scope: &'a str,
    aud: &'a str,
    exp: i64,
    iat: i64,
}

/// What Google's token endpoint answers.
#[derive(Default, Deserialize)]
#[serde(default)]
struct TokenAnswer {
    #[serde(deserialize_with = "null_as_zero")]
    access_token: String,
    #[serde(deserialize_with = "null_as_zero")]
    error: String,
    #[serde(deserialize_with = "null_as_zero")]
    error_description: String,
}

/// A Data API session: the API signed in as the service account, its email, and the property's number alone, as the Data
/// API's paths and the answers name it.
struct Session {
    api: Api,
    email: String,
    property: String,
}

impl GoogleAnalytics {
    /// The Data API of the property, signed in as the service account: one access token for the call, asked of Google with a JWT
    /// the key signs (RS256, the read-only scope, the token endpoint as audience).
    async fn session(&self) -> Result<Session, Error> {
        let property = property(&self.property)?;
        let a = account(&self.key)?;
        let token = self.sign_in(&a).await.map_err(|why| {
            format!(
                "google analytics: Google did not sign in the service account {} ({}): check that {KEY_VAR} is a key of it that was not deleted",
                a.client_email,
                redact_text(&why)
            )
        })?;
        Ok(Session {
            api: Api {
                name: NAME.to_owned(),
                base: self.data_url.clone(),
                headers: vec![("Authorization".to_owned(), format!("Bearer {token}"))],
                http: self.http.clone(),
            },
            email: a.client_email,
            property,
        })
    }

    /// The access token Google answers for a JWT the key of `a` signs, or what Google said when it refused it.
    async fn sign_in(&self, a: &ServiceAccount) -> Result<String, String> {
        let iat = self
            .now
            .checked_sub(SignedDuration::from_secs(10))
            .unwrap_or(self.now)
            .as_second();
        let claims = Claims {
            iss: &a.client_email,
            scope: SCOPE,
            aud: &a.token_uri,
            exp: iat.saturating_add(3600),
            iat,
        };
        let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid = Some(a.private_key_id.clone()).filter(|k| !k.is_empty());
        let key = jsonwebtoken::EncodingKey::from_rsa_pem(a.private_key.as_bytes())
            .map_err(|e| format!("the private_key is not an RSA key in PEM: {e}"))?;
        let jwt = jsonwebtoken::encode(&header, &claims, &key).map_err(|e| e.to_string())?;
        let form = source::encode_query(&[
            (
                "grant_type".to_owned(),
                "urn:ietf:params:oauth:grant-type:jwt-bearer".to_owned(),
            ),
            ("assertion".to_owned(), jwt),
        ]);
        let client = self.http.client().map_err(|e| e.to_string())?;
        let mut res = client
            .post(&a.token_uri)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Accept", "application/json")
            .body(form)
            .send()
            .await
            .map_err(|e| e.without_url().to_string())?;
        let mut data = Vec::new();
        while let Some(chunk) = res.chunk().await.map_err(|e| e.without_url().to_string())? {
            data.extend_from_slice(&chunk);
            if data.len() > TOKEN_ANSWER_MAX {
                return Err("Google's answer to the sign-in is too long".to_owned());
            }
        }
        let answer: TokenAnswer = serde_json::from_slice(&data).unwrap_or_default();
        if !answer.error.is_empty() {
            return Err(format!("{}: {}", answer.error, answer.error_description)
                .trim()
                .to_owned());
        }
        if !res.status().is_success() || answer.access_token.is_empty() {
            return Err(format!(
                "oauth2: cannot fetch token: {}: {}",
                res.status(),
                limit(String::from_utf8_lossy(&data).trim(), 200)
            ));
        }
        Ok(answer.access_token)
    }

    /// The answer to the Data API's `method` (`runReport`, `runRealtimeReport`) for `body`, or a GET of the property's metadata
    /// when `body` is none, with the property's number. It turns the refusals an owner or an agent can act on into what to do.
    async fn call(&self, method: &str, body: Option<&Request>) -> Result<(Vec<u8>, String), Error> {
        let s = self.session().await?;
        let answer = match body {
            None => {
                s.api
                    .get(&format!("/v1beta/properties/{}/metadata", s.property), &[])
                    .await
            }
            Some(b) => {
                s.api
                    .post_json(&format!("/v1beta/properties/{}:{method}", s.property), b)
                    .await
            }
        };
        let e = match answer {
            Ok(raw) => return Ok((raw, s.property)),
            Err(e) => e,
        };
        let what = match e.status().map(|st| st.code) {
            Some(401 | 403) => format!(
                "the owner adds {} as a Viewer of property {}: Admin, Property access management",
                s.email, s.property
            ),
            Some(429) => "the property's hourly or daily quota of the Data API is spent: wait, then ask once with fewer dimensions or a shorter range".to_owned(),
            Some(400) => "googleanalytics_metadata lists the metric and dimension names".to_owned(),
            _ => return Err(e.into()),
        };
        Err(format!("{e} ({what})").into())
    }
}

// --- the request and the answer, shared by every report ---

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Field {
    name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DateRange {
    start_date: String,
    end_date: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MinuteRange {
    start_minutes_ago: i64,
    end_minutes_ago: i64,
}

/// A FilterExpression: an and-group of exact matches, each on one dimension.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FilterExpression {
    #[serde(skip_serializing_if = "Option::is_none")]
    and_group: Option<FilterList>,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter: Option<FieldFilter>,
}

#[derive(Debug, Clone, Serialize)]
struct FilterList {
    expressions: Vec<FilterExpression>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FieldFilter {
    field_name: String,
    string_filter: StringFilter,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StringFilter {
    match_type: &'static str,
    value: String,
}

#[derive(Debug, Clone, Default, Serialize)]
struct OrderBy {
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    desc: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    metric: Option<OrderMetric>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimension: Option<OrderDimension>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrderMetric {
    metric_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrderDimension {
    dimension_name: String,
}

/// The body of runReport and runRealtimeReport: one carries date ranges, the other minute ranges.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    date_ranges: Vec<DateRange>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    minute_ranges: Vec<MinuteRange>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    dimensions: Vec<Field>,
    metrics: Vec<Field>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimension_filter: Option<FilterExpression>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    order_bys: Vec<OrderBy>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    metric_aggregations: Vec<&'static str>,
    #[serde(skip_serializing_if = "String::is_empty")]
    limit: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    keep_empty_rows: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Cell {
    #[serde(deserialize_with = "null_as_zero")]
    value: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Row {
    #[serde(deserialize_with = "null_as_zero")]
    dimension_values: Vec<Cell>,
    #[serde(deserialize_with = "null_as_zero")]
    metric_values: Vec<Cell>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Metadata {
    #[serde(deserialize_with = "null_as_zero")]
    currency_code: String,
    #[serde(deserialize_with = "null_as_zero")]
    time_zone: String,
    #[serde(deserialize_with = "null_as_zero")]
    data_loss_from_other_row: bool,
    #[serde(deserialize_with = "null_as_zero")]
    subject_to_thresholding: bool,
    #[serde(deserialize_with = "null_as_zero")]
    sampling_metadatas: Vec<Value>,
}

/// What runReport and runRealtimeReport answer.
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Response {
    #[serde(deserialize_with = "null_as_zero")]
    dimension_headers: Vec<Field>,
    #[serde(deserialize_with = "null_as_zero")]
    metric_headers: Vec<Field>,
    #[serde(deserialize_with = "null_as_zero")]
    rows: Vec<Row>,
    #[serde(deserialize_with = "null_as_zero")]
    totals: Vec<Row>,
    #[serde(deserialize_with = "null_as_zero")]
    row_count: i64,
    #[serde(deserialize_with = "null_as_zero")]
    metadata: Metadata,
}

/// Names as the Data API takes them, between `least` and `most` of them, or why they are not.
fn fields(what: &str, names: &[String], least: usize, most: usize) -> Result<Vec<Field>, String> {
    if !(least..=most).contains(&names.len()) {
        return Err(format!("{what} takes {least} to {most} names"));
    }
    names
        .iter()
        .map(|n| {
            let n = n.trim();
            if !FIELD_NAME.is_match(n) {
                return Err(format!(
                    "{what}: {:?} is not an API name such as activeUsers or customEvent:plan (googleanalytics_metadata lists them)",
                    limit(n, 40)
                ));
            }
            Ok(Field { name: n.to_owned() })
        })
        .collect()
}

/// The filter that keeps the rows whose dimensions have exactly these values, or none for none.
fn filter_of(dims: &BTreeMap<String, String>) -> Result<Option<FilterExpression>, String> {
    if dims.is_empty() {
        return Ok(None);
    }
    if dims.len() > MAX_DIMENSIONS {
        return Err(format!("where takes at most {MAX_DIMENSIONS} dimensions"));
    }
    let mut expressions = Vec::with_capacity(dims.len());
    for (dim, value) in dims {
        if !FIELD_NAME.is_match(dim) {
            return Err(format!(
                "where: {:?} is not a dimension's API name (googleanalytics_metadata lists them)",
                limit(dim, 40)
            ));
        }
        expressions.push(FilterExpression {
            and_group: None,
            filter: Some(FieldFilter {
                field_name: dim.clone(),
                string_filter: StringFilter {
                    match_type: "EXACT",
                    value: value.clone(),
                },
            }),
        });
    }
    Ok(Some(FilterExpression {
        and_group: Some(FilterList { expressions }),
        filter: None,
    }))
}

/// The rows to show for the limit an agent asked.
fn rows_of(limit: i64) -> Result<usize, String> {
    ROWS.of("limit", limit).map_err(|e| e.to_string())
}

fn parse(raw: &[u8], what: &str) -> Result<Response, Error> {
    serde_json::from_slice(raw).map_err(|e| format!("google analytics: the {what} is not JSON: {e}").into())
}

/// A report as the agent reads it: a line that says what was read, the rows under their columns, the total of each metric when
/// the rows are split, and what Google says makes the numbers less than exact.
fn report_table(r: &Response, heading: &str, rows_shown: usize) -> String {
    let cols: Vec<String> = r
        .dimension_headers
        .iter()
        .chain(&r.metric_headers)
        .map(|h| h.name.clone())
        .collect();
    let mut rows: Vec<Vec<String>> = r.rows.iter().map(cells).collect();
    if let Some(totals) = r.totals.first()
        && !r.dimension_headers.is_empty()
    {
        let mut total = vec!["total".to_owned()];
        total.resize(r.dimension_headers.len(), String::new());
        total.extend(totals.metric_values.iter().map(|v| v.value.clone()));
        rows.push(total);
    }
    let mut first = heading.to_owned();
    if !r.metadata.time_zone.is_empty() {
        first.push_str(&format!(", time zone {}", r.metadata.time_zone));
    }
    if !r.metadata.currency_code.is_empty() {
        first.push_str(&format!(", currency {}", r.metadata.currency_code));
    }
    let mut lines = vec![first];
    if usize::try_from(r.row_count).is_ok_and(|n| n > r.rows.len()) {
        lines.push(format!(
            "{} rows in all, the first {} shown: raise limit or narrow with where",
            r.row_count,
            r.rows.len()
        ));
    }
    if r.metadata.subject_to_thresholding {
        lines.push(
            "subject to thresholding: Google withholds rows of few users, so small numbers may be missing".to_owned(),
        );
    }
    if r.metadata.data_loss_from_other_row {
        lines.push(
            "some rows are rolled into (other): the property has more distinct values than a report keeps".to_owned(),
        );
    }
    if !r.metadata.sampling_metadatas.is_empty() {
        lines.push("sampled: the numbers are estimates from part of the events".to_owned());
    }
    lines.push(table(&cols, &rows, rows_shown.saturating_add(1)));
    mask_emails(&lines.join("\n"))
}

fn cells(row: &Row) -> Vec<String> {
    row.dimension_values
        .iter()
        .chain(&row.metric_values)
        .map(|v| v.value.clone())
        .collect()
}

// --- report ---

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReportInput {
    /// 1 to 10 metric API names, such as activeUsers, newUsers, sessions, screenPageViews, eventCount, keyEvents, totalRevenue
    metrics: Vec<String>,
    /// 0 to 9 dimension API names to split by, such as date, country, deviceCategory, pagePath, sessionSource, eventName
    #[serde(default)]
    dimensions: Vec<String>,
    /// first day: 2026-10-01, NdaysAgo such as 28daysAgo, yesterday or today, in the property's time zone; default 7daysAgo
    #[serde(default)]
    start: String,
    /// last day, same forms; default today. The last 48 hours may still grow as Google processes them
    #[serde(default)]
    end: String,
    /// keep only rows whose dimension has exactly this value, such as {"eventName":"purchase","country":"Vietnam"}
    #[serde(default, rename = "where")]
    r#where: BTreeMap<String, String>,
    /// a metric or dimension of the report to sort by; default the first metric, largest first, or date when date is a dimension
    #[serde(default)]
    order_by: String,
    /// smallest first; dates oldest first is the default for date
    #[serde(default)]
    ascending: bool,
    /// most rows, 1 to 100; default 25
    #[serde(default)]
    limit: i64,
}

impl GoogleAnalytics {
    async fn report(self, input: ReportInput) -> Result<String, Error> {
        let (req, rows) = report_request(input)?;
        let (raw, property) = self.call("runReport", Some(&req)).await?;
        let r = parse(&raw, "report")?;
        let days = req.date_ranges.first().map_or((String::new(), String::new()), |d| {
            (d.start_date.clone(), d.end_date.clone())
        });
        Ok(report_table(
            &r,
            &format!("property {property}, {} to {}", days.0, days.1),
            rows,
        ))
    }
}

/// The runReport an agent's input asks and the rows it shows, or why it is not one.
fn report_request(input: ReportInput) -> Result<(Request, usize), String> {
    let metrics = fields("metrics", &input.metrics, 1, MAX_METRICS)?;
    let dims = fields("dimensions", &input.dimensions, 0, MAX_DIMENSIONS)?;
    let or = |s: &str, default: &str| {
        let s = s.trim();
        if s.is_empty() { default.to_owned() } else { s.to_owned() }
    };
    let days = DateRange {
        start_date: or(&input.start, "7daysAgo"),
        end_date: or(&input.end, "today"),
    };
    for d in [&days.start_date, &days.end_date] {
        if !DAY.is_match(d) {
            return Err(format!(
                "{:?} is not a day: use 2026-10-30, 28daysAgo, yesterday or today",
                limit(d, 40)
            ));
        }
    }
    let filter = filter_of(&input.r#where)?;
    let rows = rows_of(input.limit)?;
    let order = order(&input.order_by, input.ascending, &metrics, &dims)?;
    let aggregations = if dims.is_empty() { vec![] } else { vec!["TOTAL"] };
    let req = Request {
        date_ranges: vec![days],
        dimensions: dims,
        metrics,
        dimension_filter: filter,
        order_bys: vec![order],
        metric_aggregations: aggregations,
        limit: rows.to_string(),
        ..Default::default()
    };
    Ok((req, rows))
}

/// How the rows are sorted: by the field named, or by date when the rows are days, or by the first metric, largest first.
fn order(name: &str, ascending: bool, metrics: &[Field], dims: &[Field]) -> Result<OrderBy, String> {
    let has = |list: &[Field], n: &str| list.iter().any(|f| f.name == n);
    let name = name.trim();
    if name.is_empty() && has(dims, "date") {
        return Ok(OrderBy {
            dimension: Some(OrderDimension {
                dimension_name: "date".to_owned(),
            }),
            ..Default::default()
        });
    }
    let name = if name.is_empty() {
        metrics.first().map(|f| f.name.as_str()).unwrap_or_default()
    } else {
        name
    };
    if has(metrics, name) {
        return Ok(OrderBy {
            desc: !ascending,
            metric: Some(OrderMetric {
                metric_name: name.to_owned(),
            }),
            ..Default::default()
        });
    }
    if has(dims, name) {
        return Ok(OrderBy {
            desc: !ascending,
            dimension: Some(OrderDimension {
                dimension_name: name.to_owned(),
            }),
            ..Default::default()
        });
    }
    Err(format!(
        "order_by {:?} is none of the report's metrics and dimensions",
        limit(name, 40)
    ))
}

// --- realtime ---

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RealtimeInput {
    /// metric API names; default activeUsers. Realtime takes activeUsers, eventCount, keyEvents and screenPageViews
    #[serde(default)]
    metrics: Vec<String>,
    /// dimension API names to split by, such as country, city, deviceCategory, platform, unifiedScreenName, eventName, minutesAgo
    #[serde(default)]
    dimensions: Vec<String>,
    /// how many of the last minutes, 1 to 30; default 30
    #[serde(default)]
    minutes: i64,
    /// keep only rows whose dimension has exactly this value
    #[serde(default, rename = "where")]
    r#where: BTreeMap<String, String>,
    /// most rows, 1 to 100; default 25
    #[serde(default)]
    limit: i64,
}

impl GoogleAnalytics {
    async fn realtime(self, input: RealtimeInput) -> Result<String, Error> {
        let names = if input.metrics.is_empty() {
            vec!["activeUsers".to_owned()]
        } else {
            input.metrics
        };
        let metrics = fields("metrics", &names, 1, MAX_METRICS)?;
        let dims = fields("dimensions", &input.dimensions, 0, MAX_DIMENSIONS)?;
        let minutes = minutes_of(input.minutes)?;
        let filter = filter_of(&input.r#where)?;
        let rows = rows_of(input.limit)?;
        let order = order("", false, &metrics, &[])?;
        let req = Request {
            minute_ranges: vec![MinuteRange {
                start_minutes_ago: minutes.saturating_sub(1),
                end_minutes_ago: 0,
            }],
            metric_aggregations: if dims.is_empty() { vec![] } else { vec!["TOTAL"] },
            dimensions: dims,
            metrics,
            dimension_filter: filter,
            order_bys: vec![order],
            limit: rows.to_string(),
            ..Default::default()
        };
        let (raw, property) = self.call("runRealtimeReport", Some(&req)).await?;
        let r = parse(&raw, "realtime report")?;
        Ok(report_table(
            &r,
            &format!("property {property}, the last {minutes} minutes"),
            rows,
        ))
    }
}

/// The last minutes a realtime read covers: `minutes`, or all a standard property keeps when it is zero.
fn minutes_of(minutes: i64) -> Result<i64, String> {
    if !(0..=REALTIME_MINUTES).contains(&minutes) {
        return Err(format!(
            "minutes is between 1 and {REALTIME_MINUTES}: realtime keeps the last {REALTIME_MINUTES} minutes"
        ));
    }
    Ok(if minutes == 0 { REALTIME_MINUTES } else { minutes })
}

// --- metadata ---

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct MetadataInput {
    /// only metrics and dimensions whose name or category contains this word, any case
    #[serde(default)]
    query: String,
    /// metrics or dimensions; default both
    #[serde(default)]
    kind: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct MetadataField {
    #[serde(deserialize_with = "null_as_zero")]
    api_name: String,
    #[serde(deserialize_with = "null_as_zero")]
    ui_name: String,
    #[serde(deserialize_with = "null_as_zero")]
    category: String,
    #[serde(rename = "type", deserialize_with = "null_as_zero")]
    kind: String,
    #[serde(rename = "customDefinition", deserialize_with = "null_as_zero")]
    custom: bool,
    #[serde(deserialize_with = "null_as_zero")]
    blocked_reasons: Vec<String>,
}

impl GoogleAnalytics {
    async fn metadata(self, input: MetadataInput) -> Result<String, Error> {
        #[derive(Default, Deserialize)]
        #[serde(default)]
        struct Catalog {
            #[serde(deserialize_with = "null_as_zero")]
            metrics: Vec<MetadataField>,
            #[serde(deserialize_with = "null_as_zero")]
            dimensions: Vec<MetadataField>,
        }
        let kind = input.kind.trim().to_lowercase();
        if !kind.is_empty() && kind != "metrics" && kind != "dimensions" {
            return Err(format!("kind {:?} is not metrics or dimensions", limit(&input.kind, 40)).into());
        }
        let (raw, _) = self.call("", None).await?;
        let m: Catalog =
            serde_json::from_slice(&raw).map_err(|e| format!("google analytics: the metadata is not JSON: {e}"))?;
        let word = input.query.trim().to_lowercase();
        let mut parts = Vec::new();
        if kind != "dimensions" {
            parts.push(format!("metrics\n{}", catalog(&m.metrics, &word, true)));
        }
        if kind != "metrics" {
            parts.push(format!("dimensions\n{}", catalog(&m.dimensions, &word, false)));
        }
        Ok(parts.join("\n\n"))
    }
}

/// The fields that contain `word`, as a table; a metric shows its type.
fn catalog(fields: &[MetadataField], word: &str, typed: bool) -> String {
    let mut cols = vec!["api name".to_owned(), "name".to_owned(), "category".to_owned()];
    if typed {
        cols.push("type".to_owned());
    }
    cols.push("flags".to_owned());
    let rows: Vec<Vec<String>> = fields
        .iter()
        .filter(|f| {
            format!("{} {} {}", f.api_name, f.ui_name, f.category)
                .to_lowercase()
                .contains(word)
        })
        .map(|f| {
            let mut row = vec![f.api_name.clone(), f.ui_name.clone(), f.category.clone()];
            if typed {
                row.push(f.kind.clone());
            }
            let mut flags = Vec::new();
            if f.custom {
                flags.push("custom".to_owned());
            }
            if !f.blocked_reasons.is_empty() {
                flags.push(format!("blocked: {}", f.blocked_reasons.join(", ")));
            }
            row.push(flags.join("; "));
            row
        })
        .collect();
    table(&cols, &rows, MAX_LISTED)
}
