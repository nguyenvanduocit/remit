//! End to end through a local OTLP sink in place of the intake: a line the server logs inside a card's turn leaves in the
//! turn's trace with its chain, a span the card's session exports to the door leaves under the turn's span, the key in
//! the header, and no credential either carried leaves with them.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Uri};
use axum::routing::post;
use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::common::v1::{KeyValue, any_value};
use opentelemetry_proto::tonic::logs::v1::LogRecord;
use opentelemetry_proto::tonic::trace::v1::Span;
use prost::Message;
use telemetry::{Correlate, attr};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::layer::SubscriberExt;

use crate::{Account, Datadog, Login, Signal, Site, Stamp};

/// What the sink took: each request's path, its headers and its body.
type Taken = Arc<Mutex<Vec<(String, HeaderMap, Bytes)>>>;

struct TheAccount;

impl Account for TheAccount {
    async fn login(&self) -> Result<Option<Login>, String> {
        Ok(Some(Login {
            site: Site::parse("datadoghq.eu").map_err(|e| e.to_string())?,
            api_key: "k-test".to_owned(),
        }))
    }
}

/// A Datadog whose intake is a local sink, and what the sink takes.
async fn to_a_sink() -> (Datadog, Taken) {
    // the server installs it before it sends; so does the test
    rustls::crypto::ring::default_provider().install_default().ok();
    let taken: Taken = Arc::default();
    let sink = Router::new()
        .route(
            "/{*path}",
            post(
                |State(t): State<Taken>, uri: Uri, headers: HeaderMap, body: Bytes| async move {
                    t.lock().unwrap().push((uri.path().to_owned(), headers, body));
                    ""
                },
            ),
        )
        .with_state(Arc::clone(&taken));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, sink).await });
    (Datadog::to(Some(base.clone()), Some(base), "test", "v1"), taken)
}

/// Opens card k_e2e's turn 7 of chain ch_e2e as chains does and logs inside it with fields.
fn a_turn(dd: &Datadog, log: impl FnOnce()) {
    let subscriber = tracing_subscriber::registry().with(Correlate).with(dd.layer());
    // in chains, as the server opens it: what this crate logs itself is not sent, lest sending feed itself
    tracing::subscriber::with_default(subscriber, || {
        let turn = tracing::info_span!(
            target: "chains::session",
            attr::TURN_SPAN,
            { attr::CHAIN_ID } = "ch_e2e",
            { attr::CARD_ID } = "k_e2e",
            { attr::TURN } = 7
        );
        let _in = turn.enter();
        log();
    });
}

/// What the card's session exports to the door, begun once the turn had: its own trace, a root span with attributes.
fn the_session_exports(dd: &Datadog, attributes: &str) {
    let began = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    )
    .unwrap();
    let session = format!(
        r#"{{"resourceSpans":[{{"resource":{{"attributes":[{{"key":"service.name","value":{{"stringValue":"claude-code"}}}}]}},
        "scopeSpans":[{{"spans":[{{"traceId":"0102030405060708090a0b0c0d0e0f10","spanId":"1111111111111111",
        "name":"claude_code.interaction","startTimeUnixNano":"{}","endTimeUnixNano":"{}","attributes":[{attributes}]}}]}}]}}]}}"#,
        began,
        began.saturating_add(1000)
    );
    let who = Stamp {
        chain_id: "ch_e2e".into(),
        card_id: "k_e2e".into(),
    };
    dd.forward(Signal::Traces, session.as_bytes(), &who).unwrap();
}

/// Runs the exporter until the sink took a request of logs and one of traces; their headers and bodies.
async fn sent(dd: &Datadog, taken: &Taken) -> [(HeaderMap, Bytes); 2] {
    let stop = CancellationToken::new();
    let running = tokio::spawn(dd.clone().run(TheAccount, stop.clone()));
    for _ in 0..100 {
        if taken.lock().unwrap().len() >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    stop.cancel();
    running.await.unwrap();
    let taken = taken.lock().unwrap();
    let paths: Vec<_> = taken.iter().map(|(p, ..)| p.as_str()).collect();
    ["/v1/logs", "/v1/traces"].map(|path| {
        let (_, headers, body) = taken
            .iter()
            .find(|(p, ..)| p == path)
            .unwrap_or_else(|| panic!("no {path} among {paths:?}"));
        (headers.clone(), body.clone())
    })
}

fn spans(body: Bytes) -> Vec<Span> {
    ExportTraceServiceRequest::decode(body)
        .unwrap()
        .resource_spans
        .into_iter()
        .flat_map(|r| r.scope_spans.into_iter().flat_map(|s| s.spans))
        .collect()
}

fn line(body: Bytes, message: &str) -> LogRecord {
    ExportLogsServiceRequest::decode(body)
        .unwrap()
        .resource_logs
        .into_iter()
        .flat_map(|r| r.scope_logs.into_iter().flat_map(|s| s.log_records))
        .find(|r| matches!(r.body.as_ref().and_then(|b| b.value.as_ref()), Some(any_value::Value::StringValue(s)) if s == message))
        .unwrap()
}

fn text<'a>(attrs: &'a [KeyValue], key: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|kv| kv.key == key)
        .and_then(|kv| match kv.value.as_ref()?.value.as_ref()? {
            any_value::Value::StringValue(s) => Some(s.as_str()),
            _ => None,
        })
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[tokio::test]
async fn a_turn_s_log_line_and_its_session_s_span_reach_the_intake_in_the_turn_s_trace() {
    let (dd, taken) = to_a_sink().await;
    a_turn(&dd, || {
        tracing::info!(target: "chains::session", n = 3, "inside the turn");
    });
    the_session_exports(&dd, "");
    let [(log_headers, logs), (trace_headers, traces)] = sent(&dd, &taken).await;
    assert!(dd.sending(), "the intake took what was sent");
    let links = dd.chain_links("ch_e2e");
    assert!(
        links
            .iter()
            .all(|(_, url)| url.starts_with("https://app.datadoghq.eu/")),
        "the site is the account's: {links:?}"
    );
    for h in [&log_headers, &trace_headers] {
        assert_eq!(h["dd-api-key"], "k-test");
        assert_eq!(h["content-type"], "application/x-protobuf");
    }
    assert_eq!(trace_headers["compute_stats"], "true");

    let spans = spans(traces);
    let turn = spans.iter().find(|s| s.name == attr::TURN_SPAN).unwrap();
    let agent = spans.iter().find(|s| s.name == "claude_code.interaction").unwrap();
    assert_eq!(text(&turn.attributes, attr::CHAIN_ID), Some("ch_e2e"));
    assert_eq!(
        agent.trace_id, turn.trace_id,
        "the session's span is in the turn's trace"
    );
    assert_eq!(agent.parent_span_id, turn.span_id, "under the turn's span");

    let line = line(logs, "inside the turn");
    assert_eq!(line.trace_id, turn.trace_id, "the line is in the turn's trace");
    assert_eq!(line.span_id, turn.span_id);
    assert_eq!(text(&line.attributes, attr::CHAIN_ID), Some("ch_e2e"));
    println!(
        "sink: log \"inside the turn\" trace_id={} span_id={} remit.chain_id={:?}",
        hex(&line.trace_id),
        hex(&line.span_id),
        text(&line.attributes, attr::CHAIN_ID)
    );
    println!(
        "sink: remit.turn span trace_id={} span_id={}; claude_code.interaction trace_id={} parent_span_id={} (was trace 0102…10, no parent)",
        hex(&turn.trace_id),
        hex(&turn.span_id),
        hex(&agent.trace_id),
        hex(&agent.parent_span_id)
    );
}

#[tokio::test]
async fn a_credential_in_a_log_field_or_a_forwarded_span_attribute_leaves_removed() {
    const TOKEN: &str = "ghp_abcdefghijklmnopqrstuvwxyz0123456789";
    const PASSWORD: &str = "hunter2-correct-horse";
    let (dd, taken) = to_a_sink().await;
    a_turn(&dd, || {
        tracing::info!(
            target: "chains::session",
            command = %format!("git push https://x:{TOKEN}@github.com/o/r"),
            api_key = PASSWORD,
            "pushed"
        );
    });
    the_session_exports(
        &dd,
        &format!(
            r#"{{"key":"tool.input","value":{{"stringValue":"curl -H 'Authorization: Bearer {TOKEN}'"}}}},
            {{"key":"db.password","value":{{"stringValue":"{PASSWORD}"}}}}"#
        ),
    );
    let [(_, logs), (_, traces)] = sent(&dd, &taken).await;
    for body in [&logs, &traces] {
        let raw = String::from_utf8_lossy(body);
        assert!(!raw.contains(TOKEN) && !raw.contains(PASSWORD), "a credential left");
    }

    let line = line(logs, "pushed");
    assert_eq!(text(&line.attributes, "api_key"), Some("[removed]"));
    let command = text(&line.attributes, "command").unwrap();
    assert!(command.contains("[token removed]@github.com"), "{command}");

    let spans = spans(traces);
    let agent = spans.iter().find(|s| s.name == "claude_code.interaction").unwrap();
    assert_eq!(text(&agent.attributes, "db.password"), Some("[removed]"));
    let input = text(&agent.attributes, "tool.input").unwrap();
    assert!(input.contains("Bearer [token removed]"), "{input}");
    println!(
        "sink: log command={command:?} api_key={:?}; span tool.input={input:?} db.password={:?}",
        text(&line.attributes, "api_key"),
        text(&agent.attributes, "db.password")
    );
}

#[tokio::test]
async fn a_release_published_reaches_the_api_as_a_deployment_naming_its_chains() {
    let (dd, taken) = to_a_sink().await;
    dd.deployed(crate::Deployment {
        id: "rl_1".into(),
        service: "billing".into(),
        version: "1.4.0".into(),
        repository_url: "https://github.com/acme/billing".into(),
        commit: "abc123".into(),
        started_at: 1_700_000_000_000_000_000,
        finished_at: 1_700_000_060_000_000_000,
        tags: vec!["remit.chain_id:ch_a".into(), "remit.release_id:rl_1".into()],
    });
    let stop = CancellationToken::new();
    let running = tokio::spawn(dd.clone().run(TheAccount, stop.clone()));
    for _ in 0..100 {
        if !taken.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    stop.cancel();
    running.await.unwrap();
    let taken = taken.lock().unwrap();
    let (path, headers, body) = taken.first().expect("the sink took the deployment");
    assert_eq!(path, "/api/v2/dora/deployment");
    assert_eq!(headers["dd-api-key"], "k-test");
    let sent: serde_json::Value = serde_json::from_slice(body).unwrap();
    let a = &sent["data"]["attributes"];
    assert_eq!(a["id"], "rl_1");
    assert_eq!(a["version"], "1.4.0");
    assert_eq!(a["started_at"], 1_700_000_000_000_000_000_i64);
    assert_eq!(a["git"]["commit_sha"], "abc123");
    assert_eq!(a["custom_tags"][0], "remit.chain_id:ch_a");
}
