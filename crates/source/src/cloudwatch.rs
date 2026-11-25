//! CloudWatch is the log store of the AWS side: Lambda and service logs live in log groups. Credentials come only through
//! the Env (AWS_REGION, AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, and AWS_SESSION_TOKEN when the key is temporary): no shared
//! config file, profile or AWS_ENDPOINT_URL is read, so the address and the identity are the credential's and an agent cannot
//! change either. CloudWatch Logs is called as AWS documents its wire protocol (JSON 1.1 over HTTPS, signed with Signature
//! Version 4), so no SDK chain can pick up anything else.
//!
//! Read only: this file calls DescribeLogGroups, FilterLogEvents, StartQuery, GetQueryResults and StopQuery, and nothing else.
//! StartQuery creates no data (a Logs Insights query is a search; the language has no write command). StopQuery only ends the
//! query this tool started when its deadline passed, so a scan the agent gave up on does not keep running and billing.

use std::collections::BTreeMap;
use std::sync::{Arc, LazyLock, OnceLock};
use std::time::{Duration, SystemTime};

use jiff::Timestamp;
use regex::Regex;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::api::{Http, cause_chain};
use crate::env::{Clock, Env, missing};
use crate::error::{BoxFuture, Error, Result, err, fail};
use crate::extension::sign_v4;
use crate::limit::Limit;
use crate::poll::rfc3339;
use crate::program::{PROGRAM_PATH, ProgramSpec};
use crate::redact::{mask_emails, redact_text, render};
use crate::source::{Provider, Shared, Tool, Var, new_tool};
use crate::text::{cut_bytes, limit, table};
use crate::when::parse_when;

/// How long an Insights query may run: well under the kernel's call timeout, so the answer (or the cleanup) still fits.
const INSIGHTS_WAIT: Duration = Duration::from_secs(22);
const INSIGHTS_POLL: Duration = Duration::from_secs(1);
/// Bounds a whole cloudwatch_logs call the same way.
const LOGS_BUDGET: Duration = Duration::from_secs(22);

const LOGS_LIMIT: Limit = Limit::new(50, 200);
const GROUPS_LIMIT: Limit = Limit::new(25, 50);
const INSIGHTS_ROWS: Limit = Limit::new(100, 1000);
const INSIGHTS_MAX_GROUPS: usize = 50;

/// Bytes of one event's message that an agent reads.
const EVENT_CUT: usize = 600;
/// How much of an AWS error an agent reads.
const ERROR_SNIPPET: usize = 300;

// A newest-first read walks the window backwards in chunks (the API returns oldest first): the first chunk is small, since
// the newest events are what an investigation needs, and a chunk that does not fit in CHUNK_PAGES pages is halved. In ms.
const FIRST_CHUNK: i64 = 10 * 60 * 1000;
const MAX_CHUNK: i64 = 24 * 60 * 60 * 1000;
const MIN_CHUNK: i64 = 60 * 1000;
const CHUNK_PAGES: usize = 10;
const PAGE_EVENTS: i32 = 1000;
/// FilterLogEvents requests of one tool call.
const LOGS_CALLS: usize = 80;

/// The version CloudWatch Logs names its operations under on the wire.
const TARGET_PREFIX: &str = "Logs_20140328.";

pub(crate) fn new(env: Env, shared: Shared) -> Arc<dyn Provider> {
    Arc::new(CloudWatch::of(env, shared))
}

/// One connection of CloudWatch Logs.
pub(crate) struct CloudWatch {
    env: Env,
    now: Clock,
    http: Http,
    region: String,
    key: String,
    secret: String,
    token: String,
    /// Insights: the pause between polls, and how long a query may run.
    poll: Duration,
    wait: Duration,
    /// The wire client, built on first use; a test sets a fake first.
    api: OnceLock<Arc<dyn LogsApi>>,
    /// Where the wire client sends its requests; none is AWS's address for the region. A test seam.
    endpoint: Option<String>,
}

impl CloudWatch {
    pub(crate) fn of(env: Env, shared: Shared) -> Self {
        Self {
            region: env.get("AWS_REGION"),
            key: env.get("AWS_ACCESS_KEY_ID"),
            secret: env.get("AWS_SECRET_ACCESS_KEY"),
            token: env.get("AWS_SESSION_TOKEN"),
            env,
            now: shared.now,
            http: shared.http,
            poll: INSIGHTS_POLL,
            wait: INSIGHTS_WAIT,
            api: OnceLock::new(),
            endpoint: None,
        }
    }

    fn client(&self) -> Arc<dyn LogsApi> {
        Arc::clone(self.api.get_or_init(|| {
            let endpoint = match &self.endpoint {
                Some(e) => Ok(e.clone()),
                None => logs_endpoint(&self.region),
            };
            Arc::new(Wire {
                endpoint,
                http: self.http.clone(),
                now: self.now.clone(),
                region: self.region.clone(),
                key: self.key.clone(),
                secret: self.secret.clone(),
                token: self.token.clone(),
            })
        }))
    }

    /// The period [start, end] a tool reads, with the default every tool shares: the last hour.
    fn window(&self, start: &str, end: &str) -> Result<(Timestamp, Timestamp)> {
        let now = self.now.now();
        let start = if start.is_empty() { "-1h" } else { start };
        let from = parse_when(start, now).map_err(|e| e.context("start"))?;
        let to = parse_when(end, now).map_err(|e| e.context("end"))?;
        if to <= from {
            return fail!("end is not after start");
        }
        Ok((from, to))
    }
}

impl Provider for CloudWatch {
    fn name(&self) -> String {
        "CloudWatch".to_owned()
    }

    fn missing(&self) -> Vec<String> {
        missing(&self.env, &["AWS_REGION", "AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY"])
    }

    fn vars(&self) -> Vec<Var> {
        vec![
            Var::plain("AWS_REGION", "the AWS region whose CloudWatch Logs are read").address(),
            Var::plain(
                "AWS_ACCESS_KEY_ID",
                "the access key id of an IAM user or role that may only read logs",
            )
            .secret(),
            Var::plain("AWS_SECRET_ACCESS_KEY", "the secret access key of that key id").secret(),
            Var::plain("AWS_SESSION_TOKEN", "the session token, only when the key is temporary")
                .secret()
                .optional(),
        ]
    }

    fn tools(self: Arc<Self>) -> Vec<Tool> {
        let groups = Arc::clone(&self);
        let logs = Arc::clone(&self);
        let insights = self;
        vec![
            new_tool(
                "cloudwatch_log_groups",
                "List CloudWatch log groups (Lambda functions log to /aws/lambda/<function>) with how long they keep logs and how much they hold. Match by name prefix (/aws/lambda/sync-) or by a word the name contains (case-sensitive). Use it to find the group name cloudwatch_logs and cloudwatch_insights need.",
                move |input: LogGroupsInput| {
                    let c = Arc::clone(&groups);
                    async move { c.log_groups(input).await }
                },
            ),
            new_tool(
                "cloudwatch_logs",
                "Read recent events of one CloudWatch log group, newest first, optionally narrowed by a filter pattern (a word, \"ERROR\", or { $.level = \"error\" } for JSON logs) and a stream prefix. Times are RFC3339 or relative to now such as -30m, -6h, -2d; default the last hour. Each event is cut short, its stream is marked, and Lambda START/END/REPORT lines are shortened to the request id, duration and memory. For counts, rankings or grouping use cloudwatch_insights instead.",
                move |input: LogsInput| {
                    let c = Arc::clone(&logs);
                    async move { c.logs(input).await }
                },
            ),
            new_tool(
                "cloudwatch_insights",
                "Run a CloudWatch Logs Insights query (fields, filter, stats, sort, parse, limit) over one or more log groups and a time window, and return the rows as a table. Use it to count, group or rank (stats count(*) by bin(5m), top errors, slowest requests) where cloudwatch_logs would return too many lines. Times are RFC3339 or relative such as -1h; default the last hour. It reads only, but Insights bills by the data it scans, so keep the window and groups as narrow as the question allows.",
                move |input: InsightsInput| {
                    let c = Arc::clone(&insights);
                    async move { c.insights(input).await }
                },
            ),
        ]
    }

    /// `aws` run with this connection's key (form env). The command line reads only what the typed tools read, and a little
    /// more of the same kind (get-log-events, sts get-caller-identity); that the key reads only is the owner's IAM policy. The
    /// environment is the whole of what the program knows: the key, the region, and settings that stop it reading a home
    /// directory of its own, a pager or the instance metadata service.
    fn program(&self) -> Option<ProgramSpec> {
        let words = |w: &[&str]| w.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        let (region, key, secret, token) = (
            self.region.clone(),
            self.key.clone(),
            self.secret.clone(),
            self.token.clone(),
        );
        Some(ProgramSpec {
            binary: "aws".to_owned(),
            allow: vec![
                words(&["logs", "describe-log-groups"]),
                words(&["logs", "filter-log-events"]),
                words(&["logs", "get-log-events"]),
                words(&["logs", "start-query"]),
                words(&["logs", "get-query-results"]),
                words(&["sts", "get-caller-identity"]),
            ],
            deny: words(&[
                "--endpoint-url",
                "--profile",
                "--region",
                "--ca-bundle",
                "--no-verify-ssl",
                "--debug",
                "--cli-input-json",
                "--cli-input-yaml",
                "--cli-auto-prompt",
            ]),
            deny_values: words(&["file://", "fileb://", "http://", "https://"]),
            env: Arc::new(move |dir: &str| {
                let mut env = vec![
                    format!("PATH={PROGRAM_PATH}"),
                    format!("HOME={dir}"),
                    format!("AWS_REGION={region}"),
                    format!("AWS_ACCESS_KEY_ID={key}"),
                    format!("AWS_SECRET_ACCESS_KEY={secret}"),
                    "AWS_PAGER=".to_owned(),
                    "AWS_EC2_METADATA_DISABLED=true".to_owned(),
                    "AWS_DEFAULT_OUTPUT=json".to_owned(),
                    "AWS_STS_REGIONAL_ENDPOINTS=regional".to_owned(),
                ];
                if !token.is_empty() {
                    env.push(format!("AWS_SESSION_TOKEN={token}"));
                }
                env
            }),
            secrets: vec![self.key.clone(), self.secret.clone(), self.token.clone()],
            egress: aws_egress(&self.region, &["logs", "sts"]),
            read_only: Vec::new(),
            verify: None,
        })
    }
}

// --- the wire ---

/// What CloudWatch Logs answers when a call fails: an AWS error with its code, or a failure to reach it at all.
#[derive(Debug, Clone)]
pub(crate) enum AwsError {
    Api { code: String, message: String },
    Other(String),
}

impl AwsError {
    pub(crate) fn api(code: &str, message: &str) -> Self {
        Self::Api {
            code: code.to_owned(),
            message: message.to_owned(),
        }
    }
}

impl std::fmt::Display for AwsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Api { code, message } => write!(f, "{code}: {message}"),
            Self::Other(text) => f.write_str(text),
        }
    }
}

type Answer<'a, T> = BoxFuture<'a, std::result::Result<T, AwsError>>;

/// The part of CloudWatch Logs this source calls. Tests fake it.
pub(crate) trait LogsApi: Send + Sync {
    fn describe_log_groups<'a>(&'a self, r: &'a DescribeLogGroupsInput) -> Answer<'a, DescribeLogGroupsOutput>;
    fn filter_log_events<'a>(&'a self, r: &'a FilterLogEventsInput) -> Answer<'a, FilterLogEventsOutput>;
    fn start_query<'a>(&'a self, r: &'a StartQueryInput) -> Answer<'a, StartQueryOutput>;
    fn get_query_results<'a>(&'a self, r: &'a QueryIdInput) -> Answer<'a, GetQueryResultsOutput>;
    fn stop_query<'a>(&'a self, r: &'a QueryIdInput) -> Answer<'a, serde_json::Value>;
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DescribeLogGroupsInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_group_name_prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_group_name_pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_group_identifiers: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct DescribeLogGroupsOutput {
    pub log_groups: Vec<LogGroup>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_token: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct LogGroup {
    pub log_group_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention_in_days: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stored_bytes: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FilterLogEventsInput {
    pub log_group_name: String,
    pub start_time: i64,
    pub end_time: i64,
    pub limit: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter_pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_stream_name_prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_token: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct FilterLogEventsOutput {
    pub events: Vec<FilteredLogEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_token: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct FilteredLogEvent {
    pub timestamp: i64,
    pub log_stream_name: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StartQueryInput {
    pub log_group_names: Vec<String>,
    pub query_string: String,
    pub start_time: i64,
    pub end_time: i64,
    pub limit: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct StartQueryOutput {
    pub query_id: String,
}

/// The input of GetQueryResults and StopQuery.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QueryIdInput {
    pub query_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct GetQueryResultsOutput {
    pub status: String,
    pub results: Vec<Vec<ResultField>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statistics: Option<QueryStatistics>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct ResultField {
    pub field: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct QueryStatistics {
    pub records_matched: f64,
    pub records_scanned: f64,
    pub bytes_scanned: f64,
}

/// The largest answer read from AWS: a bigger one is refused, never held.
const MAX_ANSWER: usize = 4 << 20;

/// CloudWatch Logs over HTTPS: every request is a POST of JSON 1.1 to the region's address, naming its operation in
/// X-Amz-Target and signed with the connection's key.
struct Wire {
    endpoint: std::result::Result<String, String>,
    http: Http,
    /// The time a request is signed at.
    now: Clock,
    region: String,
    key: String,
    secret: String,
    token: String,
}

/// How often one operation is sent, the first time included, as the AWS SDK's standard retryer does.
const ATTEMPTS: u32 = 3;

/// The error codes AWS answers a throttled or briefly failed request with: the ones the SDK's standard retryer sends again.
const RETRIED_CODES: [&str; 16] = [
    "Throttling",
    "ThrottlingException",
    "ThrottledException",
    "RequestThrottledException",
    "TooManyRequestsException",
    "ProvisionedThroughputExceededException",
    "TransactionInProgressException",
    "RequestLimitExceeded",
    "BandwidthLimitExceeded",
    "LimitExceededException",
    "RequestThrottled",
    "SlowDown",
    "PriorRequestNotComplete",
    "EC2ThrottledException",
    "RequestTimeout",
    "RequestTimeoutException",
];

impl Wire {
    /// `op` sent until it is answered, at most ATTEMPTS times: a throttled request, a 5xx or a broken connection is sent
    /// again after a jittered, doubling wait (at most 1s, then 2s), as the SDK's standard retryer does.
    async fn call<I: Serialize + Sync, O: DeserializeOwned>(
        &self,
        op: &str,
        input: &I,
    ) -> std::result::Result<O, AwsError> {
        let mut attempt: u32 = 1;
        loop {
            match self.attempt(op, input).await {
                Err((_, true)) if attempt < ATTEMPTS => {
                    tokio::time::sleep(backoff(attempt, self.now.now())).await;
                    attempt = attempt.saturating_add(1);
                }
                Err((e, _)) => return Err(e),
                Ok(o) => return Ok(o),
            }
        }
    }

    /// One sending of `op`; an error says whether sending it again may succeed.
    async fn attempt<I: Serialize + Sync, O: DeserializeOwned>(
        &self,
        op: &str,
        input: &I,
    ) -> std::result::Result<O, (AwsError, bool)> {
        let once = |e: AwsError| (e, false);
        let other = |e: &dyn std::fmt::Display| (AwsError::Other(e.to_string()), false);
        let endpoint = self.endpoint.as_ref().map_err(|e| other(e))?;
        let client = self.http.client().map_err(|e| other(&e))?;
        let body = serde_json::to_vec(input).map_err(|e| other(&e))?;
        let mut req = http::Request::post(endpoint.as_str())
            .header("content-type", "application/x-amz-json-1.1")
            .header("x-amz-target", format!("{TARGET_PREFIX}{op}"));
        // The session token is a header of its own, signed with the rest.
        if !self.token.is_empty() {
            req = req.header("x-amz-security-token", self.token.as_str());
        }
        let (mut parts, ()) = req.body(()).map_err(|e| other(&e))?.into_parts();
        sign_v4(
            &mut parts,
            &body,
            SystemTime::from(self.now.now()),
            &self.key,
            &self.secret,
            &self.region,
            "logs",
        )
        .map_err(|e| other(&e))?;
        let mut res = client
            .post(parts.uri.to_string())
            .headers(parts.headers)
            .body(body)
            .send()
            .await
            .map_err(|e| (AwsError::Other(redact_text(&cause_chain(&e.without_url()))), true))?;
        let mut data = Vec::new();
        while let Some(chunk) = res.chunk().await.map_err(|e| {
            (
                AwsError::Other(format!("reading the answer: {}", cause_chain(&e.without_url()))),
                true,
            )
        })? {
            data.extend_from_slice(&chunk);
            if data.len() > MAX_ANSWER {
                return Err(once(AwsError::Other(format!(
                    "the answer is larger than {} MiB: narrow the request",
                    MAX_ANSWER >> 20
                ))));
            }
        }
        if !res.status().is_success() {
            let e = aws_error(res.headers(), &data, res.status());
            let again = res.status().is_server_error()
                || matches!(&e, AwsError::Api { code, .. } if RETRIED_CODES.contains(&code.as_str()));
            return Err((e, again));
        }
        serde_json::from_slice(&data).map_err(|e| once(AwsError::Other(format!("the answer is not JSON: {e}"))))
    }
}

/// The wait before sending a request again after `attempt` sendings: a part of 2^(attempt-1) seconds that the fraction of
/// the second `now` falls in picks, so two connections retrying together do not retry in step.
fn backoff(attempt: u32, now: Timestamp) -> Duration {
    let ceiling = Duration::from_secs(1u64.checked_shl(attempt.saturating_sub(1)).unwrap_or(1));
    let jitter = now.subsec_nanosecond().unsigned_abs() % 1000;
    ceiling.mul_f64(f64::from(jitter) / 1000.0)
}

/// The error a failed JSON 1.1 answer carries: its code from X-Amzn-ErrorType or `__type` (either may be qualified, as in
/// `com.amazonaws.logs#AccessDeniedException` or `AccessDeniedException:http://...`), and its message.
fn aws_error(headers: &reqwest::header::HeaderMap, body: &[u8], status: reqwest::StatusCode) -> AwsError {
    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct Doc {
        #[serde(rename = "__type")]
        kind: String,
        message: String,
        #[serde(rename = "Message")]
        upper: String,
    }
    let doc: Doc = serde_json::from_slice(body).unwrap_or_default();
    let raw = headers
        .get("x-amzn-errortype")
        .and_then(|v| v.to_str().ok())
        .map_or(doc.kind.as_str(), |h| h);
    let code = raw.split(':').next().unwrap_or_default();
    let code = code.rsplit('#').next().unwrap_or_default();
    if code.is_empty() {
        return AwsError::Other(format!("{}: {}", status, redact_text(&String::from_utf8_lossy(body))));
    }
    let message = if doc.message.is_empty() { doc.upper } else { doc.message };
    AwsError::api(code, &message)
}

macro_rules! wire_call {
    ($name:ident, $op:literal, $in:ty, $out:ty) => {
        fn $name<'a>(&'a self, r: &'a $in) -> Answer<'a, $out> {
            Box::pin(self.call($op, r))
        }
    };
}

impl LogsApi for Wire {
    wire_call!(
        describe_log_groups,
        "DescribeLogGroups",
        DescribeLogGroupsInput,
        DescribeLogGroupsOutput
    );
    wire_call!(
        filter_log_events,
        "FilterLogEvents",
        FilterLogEventsInput,
        FilterLogEventsOutput
    );
    wire_call!(start_query, "StartQuery", StartQueryInput, StartQueryOutput);
    wire_call!(
        get_query_results,
        "GetQueryResults",
        QueryIdInput,
        GetQueryResultsOutput
    );
    wire_call!(stop_query, "StopQuery", QueryIdInput, serde_json::Value);
}

/// The address of CloudWatch Logs in `region`, or why it has none.
fn logs_endpoint(region: &str) -> std::result::Result<String, String> {
    aws_egress(region, &["logs"])
        .first()
        .and_then(|hp| hp.strip_suffix(":443"))
        .map(|host| format!("https://{host}"))
        .ok_or_else(|| format!("AWS_REGION {:?} is not the name of an AWS region", limit(region, 40)))
}

/// What AWS allows in a log group name; anything else cannot name a group and is refused before a call is made.
#[expect(clippy::expect_used, reason = "a constant pattern")]
static GROUP_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9_./#-]{1,512}$").expect("group name pattern"));

fn check_group(name: &str) -> Result<()> {
    if !GROUP_NAME.is_match(name) {
        return fail!(
            "log group {:?} is not a name: use cloudwatch_log_groups to find it",
            limit(name, 80)
        );
    }
    Ok(())
}

/// `n`, at most 1000, as the i32 AWS takes.
fn int32(n: usize) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

// --- cloudwatch_log_groups ---

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct LogGroupsInput {
    /// the group name starts with this, such as /aws/lambda/sync-
    #[serde(default)]
    prefix: String,
    /// the group name contains this word (case-sensitive); not together with prefix
    #[serde(default)]
    contains: String,
    /// most groups to list; default 25, at most 50
    #[serde(default)]
    limit: i64,
}

#[derive(Serialize)]
struct LogGroupRow {
    name: String,
    retention: String,
    stored: String,
}

#[derive(Serialize)]
struct LogGroupsAnswer {
    groups: Vec<LogGroupRow>,
    #[serde(skip_serializing_if = "String::is_empty")]
    note: String,
}

impl CloudWatch {
    async fn log_groups(&self, input: LogGroupsInput) -> Result<String> {
        if !input.prefix.is_empty() && !input.contains.is_empty() {
            return fail!("give prefix or contains, not both");
        }
        let api = self.client();
        let req = DescribeLogGroupsInput {
            limit: Some(int32(GROUPS_LIMIT.of("limit", input.limit)?)),
            log_group_name_prefix: Some(input.prefix.clone()).filter(|s| !s.is_empty()),
            log_group_name_pattern: Some(input.contains.clone()).filter(|s| !s.is_empty()),
            log_group_identifiers: None,
        };
        let out = api
            .describe_log_groups(&req)
            .await
            .map_err(|e| explain("logs:DescribeLogGroups", &e))?;
        let mut groups = out.log_groups;
        // A name pattern answers with names alone; the retention and size come from asking again by those names.
        if !input.contains.is_empty() && !groups.is_empty() {
            let again = DescribeLogGroupsInput {
                log_group_identifiers: Some(groups.iter().map(|g| g.log_group_name.clone()).collect()),
                ..DescribeLogGroupsInput::default()
            };
            groups = api
                .describe_log_groups(&again)
                .await
                .map_err(|e| explain("logs:DescribeLogGroups", &e))?
                .log_groups;
        }
        let mut rows: Vec<LogGroupRow> = groups
            .into_iter()
            .map(|g| LogGroupRow {
                retention: g
                    .retention_in_days
                    .map_or_else(|| "never expires".to_owned(), |d| format!("{d}d")),
                stored: byte_size(g.stored_bytes.unwrap_or(0.0)),
                name: g.log_group_name,
            })
            .collect();
        rows.sort_by(|a, b| a.name.cmp(&b.name));
        let note = if out.next_token.is_some() {
            "more groups exist than are listed; narrow by prefix or contains"
        } else if rows.is_empty() {
            "no log group matches"
        } else {
            ""
        };
        render(&LogGroupsAnswer {
            groups: rows,
            note: note.to_owned(),
        })
    }
}

/// `n` bytes as people read a size: 512 B, 4.0 KiB, 1.5 GiB.
fn byte_size(n: f64) -> String {
    const UNIT: f64 = 1024.0;
    if n < UNIT {
        return format!("{n:.0} B");
    }
    let mut f = n;
    let mut suffix = 'B';
    for s in ['K', 'M', 'G', 'T', 'P', 'E'] {
        if f < UNIT {
            break;
        }
        f /= UNIT;
        suffix = s;
    }
    format!("{f:.1} {suffix}iB")
}

// --- cloudwatch_logs ---

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct LogsInput {
    /// the log group name, such as /aws/lambda/sync-orders (cloudwatch_log_groups finds it)
    group: String,
    /// CloudWatch filter pattern: a word, "ERROR", "?timeout ?Timeout" or { $.level = "error" } for JSON logs; empty keeps every event
    #[serde(default)]
    filter: String,
    /// only streams whose name starts with this (a Lambda stream is 2026/10/30/[$LATEST]<instance>)
    #[serde(default)]
    stream_prefix: String,
    /// where the window begins (RFC3339 or -30m); default -1h
    #[serde(default)]
    start: String,
    /// where it ends; default now
    #[serde(default)]
    end: String,
    /// most events to return, newest first; default 50, at most 200
    #[serde(default)]
    limit: i64,
}

impl CloudWatch {
    async fn logs(&self, input: LogsInput) -> Result<String> {
        check_group(&input.group)?;
        if input.filter.len() > 1024 || input.stream_prefix.len() > 512 {
            return fail!("filter and stream_prefix are too long");
        }
        let (from, to) = self.window(&input.start, &input.end)?;
        let mut r = LogRead {
            api: self.client(),
            group: input.group.clone(),
            filter: input.filter.clone(),
            stream_prefix: input.stream_prefix.clone(),
            calls: LOGS_CALLS,
            deadline: tokio::time::Instant::now()
                .checked_add(LOGS_BUDGET)
                .unwrap_or_else(tokio::time::Instant::now),
        };
        let want = LOGS_LIMIT.of("limit", input.limit)?;
        let got = r.newest(from, to, want).await?;
        Ok(format_events(&input, from, to, &got))
    }
}

/// Reads one group newest first. `calls` is what is left of the request budget; past `deadline` no request is waited on,
/// and what was read by then is the answer.
struct LogRead {
    api: Arc<dyn LogsApi>,
    group: String,
    filter: String,
    stream_prefix: String,
    calls: usize,
    deadline: tokio::time::Instant,
}

/// The newest events of a window, and why fewer than asked for may be shown.
#[derive(Default)]
struct EventsRead {
    /// Newest first, at most the limit.
    events: Vec<FilteredLogEvent>,
    /// The limit was reached, so older events in the window may exist.
    more: bool,
    /// Set when the read stopped before the whole window was covered.
    note: String,
}

impl EventsRead {
    fn cut(mut self, limit: usize) -> Self {
        if self.events.len() > limit {
            self.events.truncate(limit);
            self.more = true;
        }
        self
    }
}

/// Why a scan stopped before its pages were read.
enum ScanError {
    Budget,
    Late,
    Aws(Error),
}

impl std::fmt::Display for ScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Budget => f.write_str("the request budget of this call is used"),
            Self::Late => write!(f, "the read took longer than {}s", LOGS_BUDGET.as_secs()),
            Self::Aws(e) => write!(f, "{e}"),
        }
    }
}

impl LogRead {
    /// The `limit` newest events of [from, to]. FilterLogEvents answers oldest first, so the window is walked backwards from
    /// its end in chunks until `limit` events are held: the newest events are never the ones left unread.
    async fn newest(&mut self, from: Timestamp, to: Timestamp, limit: usize) -> Result<EventsRead> {
        let mut res = EventsRead::default();
        let lo = from.as_millisecond();
        let mut hi = to.as_millisecond();
        let mut chunk = FIRST_CHUNK;
        while hi >= lo && res.events.len() < limit {
            let start = hi.saturating_sub(chunk).saturating_add(1).max(lo);
            let (mut evs, complete) = match self.scan(start, hi).await {
                Err(e) if res.events.is_empty() => {
                    return Err(match e {
                        ScanError::Aws(e) => e,
                        ScanError::Late => err!("cloudwatch: {e}: narrow the window or add a filter"),
                        ScanError::Budget => Error::msg(e.to_string()),
                    });
                }
                Err(e) => {
                    res.note = format!("stopped before reaching the start of the window ({e}); older events not read");
                    return Ok(res.cut(limit));
                }
                Ok(got) => got,
            };
            if !complete && chunk > MIN_CHUNK {
                chunk = chunk.checked_div(2).unwrap_or(MIN_CHUNK); // too much to read in one go: retry the newer half
                continue;
            }
            if !complete {
                res.note = "a minute of this group holds more matching events than can be read: narrow with filter or a shorter window".to_owned();
            }
            evs.sort_by_key(|e| std::cmp::Reverse(e.timestamp));
            res.events.append(&mut evs);
            hi = start.saturating_sub(1);
            chunk = chunk.saturating_mul(4).min(MAX_CHUNK);
        }
        let mut res = res.cut(limit);
        // The limit was met with part of the window unread.
        res.more = res.more || (res.events.len() >= limit && hi >= lo);
        Ok(res)
    }

    /// Every matching event of [lo, hi] (ms), oldest first, and whether all of them were read within CHUNK_PAGES pages.
    async fn scan(&mut self, lo: i64, hi: i64) -> std::result::Result<(Vec<FilteredLogEvent>, bool), ScanError> {
        let mut req = FilterLogEventsInput {
            log_group_name: self.group.clone(),
            start_time: lo,
            end_time: hi,
            limit: PAGE_EVENTS,
            filter_pattern: Some(self.filter.clone()).filter(|s| !s.is_empty()),
            log_stream_name_prefix: Some(self.stream_prefix.clone()).filter(|s| !s.is_empty()),
            next_token: None,
        };
        let mut evs = Vec::new();
        for _ in 0..CHUNK_PAGES {
            let Some(left) = self.calls.checked_sub(1) else {
                return Err(ScanError::Budget);
            };
            self.calls = left;
            let mut out = tokio::time::timeout_at(self.deadline, self.api.filter_log_events(&req))
                .await
                .map_err(|_| ScanError::Late)?
                .map_err(|e| ScanError::Aws(explain("logs:FilterLogEvents", &e)))?;
            evs.append(&mut out.events);
            match out.next_token {
                None => return Ok((evs, true)),
                token => req.next_token = token,
            }
        }
        Ok((evs, false))
    }
}

fn format_events(input: &LogsInput, from: Timestamp, to: Timestamp, got: &EventsRead) -> String {
    let mut b = format!("{}, {} to {}", input.group, rfc3339(from), rfc3339(to));
    if !input.filter.is_empty() {
        b.push_str(&format!(", filter {}", input.filter));
    }
    b.push_str(&format!(": {} events, newest first\n", got.events.len()));
    let mut streams: BTreeMap<&str, String> = BTreeMap::new();
    let mut order: Vec<&str> = Vec::new();
    let mut lines = String::new();
    for e in &got.events {
        let name = e.log_stream_name.as_str();
        let tag = match streams.get(name) {
            Some(t) => t.clone(),
            None => {
                let t = format!("s{}", streams.len().saturating_add(1));
                streams.insert(name, t.clone());
                order.push(name);
                t
            }
        };
        lines.push_str(&format!(
            "{} {tag} {}\n",
            millis_text(e.timestamp),
            event_line(&e.message)
        ));
    }
    for name in order {
        let tag = streams.get(name).map_or("", String::as_str);
        b.push_str(&format!("{tag} = stream {name}\n"));
    }
    b.push_str(&lines);
    if got.more {
        b.push_str(&format!(
            "[the limit was reached: older events in the window may not be shown; raise limit (at most {}), or narrow start or filter]\n",
            LOGS_LIMIT.max
        ));
    } else if got.events.is_empty() {
        b.push_str("[no events match in this window]\n");
    }
    if !got.note.is_empty() {
        b.push_str(&format!("[{}]\n", got.note));
    }
    b.trim_end_matches('\n').to_owned()
}

/// `ms` since the epoch as 2006-01-02T15:04:05.000Z.
fn millis_text(ms: i64) -> String {
    let Ok(t) = Timestamp::from_millisecond(ms) else {
        return ms.to_string();
    };
    format!("{}.{:03}Z", t.strftime("%Y-%m-%dT%H:%M:%S"), ms.rem_euclid(1000))
}

// --- one event ---

#[expect(clippy::expect_used, reason = "constant patterns")]
static LAMBDA: LazyLock<[Regex; 4]> = LazyLock::new(|| {
    [
        Regex::new(r"^(START|END) RequestId: (\S+)").expect("start/end"),
        Regex::new(r"^REPORT RequestId: (\S+)").expect("report"),
        Regex::new(r"\t(Duration|Billed Duration|Init Duration|Memory Size|Max Memory Used): ([\d.]+) (ms|MB)")
            .expect("field"),
        Regex::new(r"\t(Status|Error Type): (\S+)").expect("status"),
    ]
});

/// One event as a single short line: Lambda's platform lines reduced to what an investigation uses, any other message as
/// `one_line` leaves it.
fn event_line(msg: &str) -> String {
    let [start_end, report, _, _] = &*LAMBDA;
    if let Some(m) = start_end.captures(msg) {
        return format!("{} req={}", &m[1], &m[2]);
    }
    if let Some(m) = report.captures(msg) {
        return report_line(&m[1], msg);
    }
    one_line(msg)
}

/// A free-text value as one short line: newlines (a stack trace) joined with " | ", cut at EVENT_CUT bytes, emails masked.
pub(crate) fn one_line(msg: &str) -> String {
    let msg = msg
        .trim_end_matches([' ', '\t', '\r', '\n'])
        .replace('\r', "")
        .replace('\n', " | ")
        .replace('\t', " ");
    if msg.len() > EVENT_CUT {
        let head = cut_bytes(&msg, EVENT_CUT);
        return mask_emails(&format!("{head}... [+{} bytes]", msg.len().saturating_sub(head.len())));
    }
    mask_emails(&msg)
}

/// REPORT's request id, durations, memory and failure status.
fn report_line(id: &str, msg: &str) -> String {
    let [_, _, field, status] = &*LAMBDA;
    let f: BTreeMap<String, String> = field
        .captures_iter(msg)
        .map(|m| (m[1].to_owned(), format!("{}{}", &m[2], &m[3])))
        .collect();
    let mut parts = vec![format!("REPORT req={id}")];
    for (key, label) in [
        ("Duration", "duration"),
        ("Billed Duration", "billed"),
        ("Init Duration", "init"),
    ] {
        if let Some(v) = f.get(key) {
            parts.push(format!("{label} {v}"));
        }
    }
    if let Some(used) = f.get("Max Memory Used") {
        let size = f.get("Memory Size").map_or("", String::as_str);
        parts.push(format!("mem {}/{size}", used.trim_end_matches("MB")));
    }
    for m in status.captures_iter(msg) {
        parts.push(format!("{}={}", m[1].replace(' ', "").to_lowercase(), &m[2]));
    }
    parts.join(" ")
}

// --- cloudwatch_insights ---

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub(crate) struct InsightsInput {
    /// one or more log group names (at most 50); cloudwatch_log_groups finds them
    groups: Vec<String>,
    /// a Logs Insights query, for example: fields @timestamp, @message | filter @message like /ERROR/ | stats count(*) by bin(5m)
    query: String,
    /// where the window begins (RFC3339 or -1h); default -1h
    #[serde(default)]
    start: String,
    /// where it ends; default now
    #[serde(default)]
    end: String,
    /// most rows to return; default 100, at most 1000
    #[serde(default)]
    rows: i64,
}

impl CloudWatch {
    async fn insights(&self, input: InsightsInput) -> Result<String> {
        if input.query.trim().is_empty() || input.query.len() > 10_000 {
            return fail!("query is empty or longer than 10000 characters");
        }
        if input.groups.is_empty() || input.groups.len() > INSIGHTS_MAX_GROUPS {
            return fail!("name between 1 and {INSIGHTS_MAX_GROUPS} log groups");
        }
        for g in &input.groups {
            check_group(g)?;
        }
        let (from, to) = self.window(&input.start, &input.end)?;
        let rows = INSIGHTS_ROWS.of("rows", input.rows)?;
        let api = self.client();
        let started = api
            .start_query(&StartQueryInput {
                log_group_names: input.groups.clone(),
                query_string: input.query.clone(),
                start_time: from.as_second(),
                end_time: to.as_second(),
                limit: int32(rows),
            })
            .await
            .map_err(|e| explain("logs:StartQuery", &e))?;
        let out = self.wait_for(&api, &started.query_id).await?;
        Ok(insights_table(&out, rows))
    }

    /// Polls the query until it completes, fails, or `wait` passes; a query still running then is stopped, so it does not
    /// keep scanning (and billing) after the agent has been told to narrow it.
    async fn wait_for(&self, api: &Arc<dyn LogsApi>, id: &str) -> Result<GetQueryResultsOutput> {
        let deadline = tokio::time::Instant::now()
            .checked_add(self.wait)
            .ok_or_else(|| err!("cloudwatch: the wait is too long"))?;
        let q = QueryIdInput {
            query_id: id.to_owned(),
        };
        let mut running = StopOnDrop {
            api: Some(Arc::clone(api)),
            q: q.clone(),
        };
        let out = self.poll_until(api.as_ref(), &q, deadline).await;
        running.api = None; // ended, or already stopped by `abandon`
        out
    }

    async fn poll_until(
        &self,
        api: &dyn LogsApi,
        q: &QueryIdInput,
        deadline: tokio::time::Instant,
    ) -> Result<GetQueryResultsOutput> {
        let still_running = || {
            err!(
                "cloudwatch: the query was still running after {}: narrow the window or the groups, or add a filter before stats",
                crate::when::duration_text(self.wait)
            )
        };
        loop {
            let out = match tokio::time::timeout_at(deadline, api.get_query_results(q)).await {
                Err(_) => return Err(abandon(api, q, still_running()).await),
                Ok(Err(e)) => return Err(abandon(api, q, explain("logs:GetQueryResults", &e)).await),
                Ok(Ok(out)) => out,
            };
            match out.status.as_str() {
                "Complete" => return Ok(out),
                "Failed" | "Cancelled" | "Timeout" | "Unknown" => {
                    return fail!(
                        "cloudwatch: the query ended as {}: check its syntax, or narrow the window",
                        out.status
                    );
                }
                _ => {}
            }
            if tokio::time::timeout_at(deadline, tokio::time::sleep(self.poll))
                .await
                .is_err()
            {
                return Err(abandon(api, q, still_running()).await);
            }
        }
    }
}

/// A query started at AWS that the call no longer waits on when the call itself is dropped (the agent's request was
/// cancelled): it is stopped on the runtime so it does not keep scanning and billing. Disarmed by taking `api`.
struct StopOnDrop {
    api: Option<Arc<dyn LogsApi>>,
    q: QueryIdInput,
}

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        let Some(api) = self.api.take() else {
            return;
        };
        let Ok(rt) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let q = std::mem::take(&mut self.q);
        rt.spawn(async move {
            // Nobody is left to tell of the outcome: the stop is best effort, as `abandon`'s is.
            let _stopped = tokio::time::timeout(Duration::from_secs(3), api.stop_query(&q)).await;
        });
    }
}

/// Stops the query and returns `cause`. The stop is best effort and is told apart when it fails.
async fn abandon(api: &dyn LogsApi, q: &QueryIdInput, cause: Error) -> Error {
    let stopped = tokio::time::timeout(Duration::from_secs(3), api.stop_query(q)).await;
    let failed = match stopped {
        Ok(Ok(_)) => return cause,
        Ok(Err(e)) => e.to_string(),
        Err(_) => "the stop took longer than 3s".to_owned(),
    };
    err!(
        "{cause} (and the query could not be stopped, so it may still run at AWS: {})",
        limit(&failed, 120)
    )
}

/// The result as a table; columns are the fields in the order they first appear, and @ptr, the internal pointer Insights
/// adds to raw-event queries, is left out.
fn insights_table(out: &GetQueryResultsOutput, max_rows: usize) -> String {
    let mut cols: Vec<String> = Vec::new();
    for row in &out.results {
        for f in row {
            if f.field != "@ptr" && !cols.contains(&f.field) {
                cols.push(f.field.clone());
            }
        }
    }
    let rows: Vec<Vec<String>> = out
        .results
        .iter()
        .map(|row| {
            let cells: BTreeMap<&str, String> = row.iter().map(|f| (f.field.as_str(), one_line(&f.value))).collect();
            cols.iter()
                .map(|name| cells.get(name.as_str()).cloned().unwrap_or_default())
                .collect()
        })
        .collect();
    let mut text = table(&cols, &rows, max_rows);
    if let Some(s) = &out.statistics {
        text.push_str(&format!(
            "\n[scanned {:.0} records ({}), {:.0} matched]",
            s.records_scanned,
            byte_size(s.bytes_scanned),
            s.records_matched
        ));
    }
    text
}

// --- errors ---

#[expect(clippy::expect_used, reason = "a constant pattern")]
static DENIED_ACTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"not authorized to perform: ([A-Za-z0-9:*]+)").expect("denied action pattern"));

/// `e` as an agent should read it. A denial is the owner's IAM policy (the machine's key has guardrails on purpose), not a
/// defect to work around; an expired or wrong key is the owner's to renew.
fn explain(op: &str, e: &AwsError) -> Error {
    let (code, message) = match e {
        AwsError::Other(text) => return err!("cloudwatch: {op}: {}", limit(text, ERROR_SNIPPET)),
        AwsError::Api { code, message } => (code.as_str(), message.as_str()),
    };
    match code {
        "AccessDeniedException" | "AccessDenied" | "UnauthorizedOperation" => {
            let action = DENIED_ACTION
                .captures(message)
                .and_then(|m| m.get(1))
                .map_or(op, |m| m.as_str());
            err!(
                "cloudwatch: AWS denied {action}. That is the owner's IAM policy, not a defect: do not retry or work around it; tell the owner what you needed, or use another tool"
            )
        }
        "ExpiredTokenException"
        | "ExpiredToken"
        | "UnrecognizedClientException"
        | "InvalidClientTokenId"
        | "InvalidSignatureException"
        | "SignatureDoesNotMatch" => err!(
            "cloudwatch: AWS refused the credential ({code}): the owner must renew AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY and AWS_SESSION_TOKEN"
        ),
        "ResourceNotFoundException" => err!(
            "cloudwatch: {op}: the log group or query does not exist in this region: cloudwatch_log_groups lists the groups"
        ),
        _ => err!("cloudwatch: {op}: {code}: {}", limit(message, ERROR_SNIPPET)),
    }
}

// --- the program's egress ---

#[expect(clippy::expect_used, reason = "a constant pattern")]
static AWS_REGION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z]{2}(-[a-z]+)+-[0-9]$").expect("region pattern"));

/// The addresses of the services in a region, none for a region that is not a region's name: the key then reaches nothing.
pub(crate) fn aws_egress(region: &str, services: &[&str]) -> Vec<String> {
    if !AWS_REGION.is_match(region) {
        return Vec::new();
    }
    let domain = if region.starts_with("cn-") {
        "amazonaws.com.cn"
    } else {
        "amazonaws.com"
    };
    services.iter().map(|s| format!("{s}.{region}.{domain}:443")).collect()
}
