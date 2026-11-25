//! The correlation attributes each span of the server carries (ADR-0119, `attr::ALL`): those it set where it opened, and
//! its parents' (`Correlate`, a layer of the server's `tracing` subscriber installed with or without Datadog). Every
//! record inside a span says what it is about from them: the exporter's logs, the server's issues, a child's trace.

use tracing::field::{Field, Visit};
use tracing::{Subscriber, span};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

use crate::attr;

/// The value of a correlation attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Text(String),
    Int(i64),
}

impl Value {
    pub fn text(&self) -> String {
        match self {
            Value::Text(s) => s.clone(),
            Value::Int(n) => n.to_string(),
        }
    }
}

/// The correlation attributes a span set where it opened, its parents' included: what every record inside it says it is
/// about. `Correlate` keeps it on each span.
#[derive(Debug, Clone, Default)]
pub struct Correlation(Vec<(&'static str, Value)>);

impl Correlation {
    fn set(&mut self, name: &'static str, value: Value) {
        self.0.retain(|(n, _)| *n != name);
        self.0.push((name, value));
    }

    /// The value of correlation attribute key, as text.
    pub fn get(&self, key: &str) -> Option<String> {
        self.0.iter().find(|(n, _)| *n == key).map(|(_, v)| v.text())
    }

    /// Every correlation attribute set, by name.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &Value)> {
        self.0.iter().map(|(n, v)| (*n, v))
    }
}

impl Visit for Correlation {
    fn record_i64(&mut self, field: &Field, value: i64) {
        if let Some(name) = correlation_name(field) {
            self.set(name, Value::Int(value));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if let Some(name) = correlation_name(field) {
            self.set(name, Value::Text(value.to_owned()));
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if let Some(name) = correlation_name(field) {
            self.set(name, Value::Text(format!("{value:?}")));
        }
    }
}

fn correlation_name(field: &Field) -> Option<&'static str> {
    attr::ALL.into_iter().find(|n| *n == field.name())
}

/// Keeps each span's `Correlation`, with or without Datadog: the server's issues and its children read it too.
pub struct Correlate;

impl<S> Layer<S> for Correlate
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        let mut c = span
            .parent()
            .and_then(|p| p.extensions().get::<Correlation>().cloned())
            .unwrap_or_default();
        attrs.record(&mut c);
        span.extensions_mut().insert(c);
    }

    fn on_record(&self, id: &span::Id, values: &span::Record<'_>, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        if let Some(c) = span.extensions_mut().get_mut::<Correlation>() {
            values.record(c);
        }
    }
}
