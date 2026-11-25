//! What a page reported failing (ADR-0119), sent as the page's own errors: a log record each, of `service:remit-web`,
//! with `error.kind` and `error.message` so Error Tracking groups it as an issue, tagged with the chain the page showed (as
//! the server let the session name it) and the page's core (`session.id`, a random name): nothing that names a person.
//! The core counts a minute's faults without their times: a record's time is when the server took the report, and
//! `remit.page.fault_count` says how often it happened in that minute.
//! The page sends nothing to Datadog itself: no script of Datadog's runs in it and it holds no token. Pure but for `Datadog::page_faults`, which queues what this makes.

use std::sync::Arc;

use opentelemetry_proto::tonic::common::v1::{AnyValue, InstrumentationScope, any_value};
use opentelemetry_proto::tonic::logs::v1::LogRecord;
use telemetry::attr;
use wire::app::{FaultKind, PageFault};

use crate::layer::now_ns;
use crate::queue::{Item, Record};
use crate::{Datadog, int, resource, text};

/// The service a page reports as.
pub const SERVICE_WEB: &str = "remit-web";

/// The severity of an error (OpenTelemetry's ERROR).
const ERROR: i32 = 17;

impl Datadog {
    /// Queues faults, what a page found failing, as that page's errors, its core named session (empty: none said); nothing
    /// while nothing is sent.
    pub fn page_faults(&self, session: &str, faults: &[PageFault]) {
        let i = &self.inner;
        if !i.wanted() {
            return;
        }
        let resource = Arc::new(resource(SERVICE_WEB, &i.env, &i.version));
        let scope = Arc::new(InstrumentationScope {
            name: SERVICE_WEB.to_owned(),
            version: i.version.clone(),
            ..InstrumentationScope::default()
        });
        let now = now_ns();
        i.queue.put(faults.iter().map(|f| Item {
            resource: Arc::clone(&resource),
            scope: Arc::clone(&scope),
            record: Record::Log(log(session, f, now)),
        }));
    }
}

/// fault as the log record of an error, at now.
fn log(session: &str, f: &PageFault, now: u64) -> LogRecord {
    // the kind as the contract names it (`uncaught`, `rejection`, `call`)
    let kind = serde_json::to_value(f.kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default();
    let place = match f.kind {
        FaultKind::Uncaught | FaultKind::Rejection => "code.filepath",
        FaultKind::Call => "rpc.method",
    };
    let mut attributes = vec![
        text("error.kind", &kind),
        text("error.message", &f.message),
        int("remit.page.fault_count", f.count),
    ];
    if !session.is_empty() {
        attributes.push(text("session.id", session));
    }
    if !f.at.is_empty() {
        attributes.push(text(place, &f.at));
    }
    if !f.chain.is_empty() {
        attributes.push(text(attr::CHAIN_ID, &f.chain));
    }
    LogRecord {
        time_unix_nano: now,
        observed_time_unix_nano: now,
        severity_number: ERROR,
        severity_text: "ERROR".to_owned(),
        body: Some(AnyValue {
            value: Some(any_value::Value::StringValue(f.message.clone())),
        }),
        attributes,
        ..LogRecord::default()
    }
}
