//! What the server's `tracing` subscriber gains: its spans exported through OpenTelemetry, and every event a log record
//! in its span's trace (`trace_id`, `span_id`) carrying the correlation attributes its spans set where they opened
//! (`telemetry::attr`), so Datadog shows a span's logs and a log's trace, and one search on a chain finds both. An ERROR
//! carries `error.kind` and `error.message`, so Error Tracking makes it an issue tagged with its chain: no stack, as the
//! one a log line could take is where it was logged, not where it failed, and taking it would cost every error its time
//! on the worker that logged it. Nothing is made while the exporter wants nothing (no key).
//! It also records each card's turn as its span is first entered (`Turns`), for the `/otlp` door. It reads each span's
//! `telemetry::Correlation`, which `telemetry::Correlate`, installed with or without Datadog, keeps.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use opentelemetry::trace::{TraceContextExt, TracerProvider};
use opentelemetry_proto::tonic::common::v1::{AnyValue, KeyValue, any_value};
use opentelemetry_proto::tonic::logs::v1::LogRecord;
use opentelemetry_sdk::trace::SdkTracerProvider;
use telemetry::{Correlation, attr, correlate};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber, span};
use tracing_subscriber::Layer;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

use crate::queue::{Item, Record};
use crate::spans::Queued;
use crate::{Datadog, Inner, SCOPE, Turn, int, text};

/// The crates whose events the exporter's own work makes (its HTTP calls, its warnings): sent, they would make more.
const OWN: [&str; 6] = ["datadog", "hyper", "reqwest", "h2", "rustls", "opentelemetry"];

fn own(target: &str) -> bool {
    let krate = target.split("::").next().unwrap_or(target);
    OWN.iter().any(|o| krate.starts_with(o))
}

impl Datadog {
    /// The layer the server's subscriber gains (above).
    pub fn layer<S>(&self) -> impl Layer<S> + Send + Sync + 'static
    where
        S: Subscriber + for<'a> LookupSpan<'a> + Send + Sync,
    {
        let provider = SdkTracerProvider::builder()
            .with_span_processor(Queued(Arc::clone(&self.inner)))
            // every event is a log record of its own, in its span's trace: as a span event too, it would go twice
            .with_max_events_per_span(0)
            .build();
        let otel = tracing_opentelemetry::layer()
            .with_tracer(provider.tracer(SCOPE))
            .with_threads(false);
        otel.and_then(Logs(Arc::clone(&self.inner)))
            .with_filter(filter_fn(|m| !own(m.target())))
    }
}

struct Logs(Arc<Inner>);

/// The turn span was recorded in `Turns`.
struct Registered;

impl<S> Layer<S> for Logs
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        if span.name() != attr::TURN_SPAN {
            return;
        }
        let (card, turn) = {
            let ext = span.extensions();
            if ext.get::<Registered>().is_some() {
                return;
            }
            let c = ext.get::<Correlation>();
            (
                c.and_then(|c| c.get(attr::CARD_ID)).unwrap_or_default(),
                c.and_then(|c| c.get(attr::TURN))
                    .and_then(|t| t.parse().ok())
                    .unwrap_or_default(),
            )
        };
        // entered, the span's OpenTelemetry context is the current one (tracing-opentelemetry activates it)
        let cx = opentelemetry::Context::current();
        let sc = cx.span().span_context().clone();
        if card.is_empty() || !sc.is_valid() {
            return;
        }
        self.0.turns.began(
            &card,
            Turn {
                start_ns: now_ns(),
                turn,
                trace_id: sc.trace_id().to_bytes(),
                span_id: sc.span_id().to_bytes(),
            },
        );
        span.extensions_mut().insert(Registered);
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        if !self.0.wanted() {
            return;
        }
        let meta = event.metadata();
        let mut fields = Fields::default();
        event.record(&mut fields);
        let mut attributes = fields.attrs;
        attributes.push(text("logger.name", meta.target()));
        if let Some(file) = meta.file() {
            attributes.push(text("code.filepath", file));
        }
        if let Some(line) = meta.line() {
            attributes.push(int("code.lineno", i64::from(line)));
        }
        if let Some(span) = ctx.event_span(event)
            && let Some(c) = span.extensions().get::<Correlation>()
        {
            attributes.extend(c.iter().map(|(name, value)| match value {
                correlate::Value::Text(s) => text(name, s),
                correlate::Value::Int(n) => int(name, *n),
            }));
        }
        let level = *meta.level();
        if level == Level::ERROR {
            let message = match &fields.error {
                Some(err) if !fields.message.is_empty() => format!("{}: {err}", fields.message),
                Some(err) => err.clone(),
                None => fields.message.clone(),
            };
            attributes.push(text("error.kind", meta.target()));
            attributes.push(text("error.message", &message));
        }
        let cx = opentelemetry::Context::current();
        let sc = cx.span().span_context().clone();
        let (trace_id, span_id, flags) = if sc.is_valid() {
            (
                sc.trace_id().to_bytes().to_vec(),
                sc.span_id().to_bytes().to_vec(),
                u32::from(sc.trace_flags().to_u8()),
            )
        } else {
            (Vec::new(), Vec::new(), 0)
        };
        let now = now_ns();
        let record = LogRecord {
            time_unix_nano: now,
            observed_time_unix_nano: now,
            severity_number: severity(level),
            severity_text: level.as_str().to_owned(),
            body: Some(AnyValue {
                value: Some(any_value::Value::StringValue(fields.message)),
            }),
            attributes,
            trace_id,
            span_id,
            flags,
            ..LogRecord::default()
        };
        self.0.queue.put([Item {
            resource: Arc::clone(&self.0.server),
            scope: Arc::clone(&self.0.scope),
            record: Record::Log(record),
        }]);
    }
}

/// OpenTelemetry's severity number of level.
fn severity(level: Level) -> i32 {
    match level {
        Level::TRACE => 1,
        Level::DEBUG => 5,
        Level::INFO => 9,
        Level::WARN => 13,
        Level::ERROR => 17,
    }
}

pub(crate) fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| u64::try_from(d.as_nanos()).ok())
        .unwrap_or_default()
}

/// An event's fields: its message, the error it names (`err` or `error`), and the rest as attributes.
#[derive(Default)]
struct Fields {
    message: String,
    error: Option<String>,
    attrs: Vec<KeyValue>,
}

impl Fields {
    fn put(&mut self, field: &Field, value: AnyValue) {
        let as_text = || match &value.value {
            Some(any_value::Value::StringValue(s)) => s.clone(),
            _ => String::new(),
        };
        match field.name() {
            "message" => self.message = as_text(),
            name @ ("err" | "error") => {
                self.error = Some(as_text());
                self.attrs.push(KeyValue {
                    key: name.to_owned(),
                    value: Some(value),
                    ..KeyValue::default()
                });
            }
            name => self.attrs.push(KeyValue {
                key: name.to_owned(),
                value: Some(value),
                ..KeyValue::default()
            }),
        }
    }
}

fn any(v: any_value::Value) -> AnyValue {
    AnyValue { value: Some(v) }
}

impl Visit for Fields {
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.put(field, any(any_value::Value::DoubleValue(value)));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.put(field, any(any_value::Value::IntValue(value)));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        let v = i64::try_from(value).map_or_else(
            |_| any_value::Value::StringValue(value.to_string()),
            any_value::Value::IntValue,
        );
        self.put(field, any(v));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.put(field, any(any_value::Value::BoolValue(value)));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.put(field, any(any_value::Value::StringValue(value.to_owned())));
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.put(field, any(any_value::Value::StringValue(value.to_string())));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.put(field, any(any_value::Value::StringValue(format!("{value:?}"))));
    }
}
