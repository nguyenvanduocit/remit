//! A child process Remit starts for a piece of work (ADR-0119): a check, a release's step, a source's program or script.
//! It runs in a span of its own, `remit.child`, under the span of that work, which records how it ended; and it is told
//! that span (`childenv::trace`): its W3C trace context and the work's correlation attributes, so what it passes on joins
//! that trace and names its chain.

use opentelemetry::trace::TraceContextExt as _;
use tracing::Span;
use tracing::field::Empty;
use tracing_opentelemetry::OpenTelemetrySpanExt as _;
use tracing_subscriber::Registry;
use tracing_subscriber::registry::LookupSpan as _;

use crate::attr;
use crate::correlate::{Correlate, Correlation};

/// A child's span.
#[derive(Debug, Clone)]
pub struct Child {
    span: Span,
}

impl Child {
    /// The span of a child running program, under the current span, for release when it is one's step (empty: none).
    pub fn open(program: &str, release: &str) -> Self {
        let span = tracing::info_span!(
            attr::CHILD_SPAN,
            program,
            { attr::RELEASE_ID } = Empty,
            exit_code = Empty,
            timed_out = Empty,
            otel.status_code = Empty,
        );
        if !release.is_empty() {
            span.record(attr::RELEASE_ID, release);
        }
        Self { span }
    }

    /// The span, for the child's run to be instrumented with.
    pub fn span(&self) -> &Span {
        &self.span
    }

    /// env, the child's `NAME=value` entries, told of its span (`childenv::trace`).
    pub fn tell(&self, env: Vec<String>) -> Vec<String> {
        childenv::trace(env, &trace_of(&self.span))
    }

    /// The child ended with exit_code, or the jail ended it at its deadline: a non-zero exit or a timeout marks the span
    /// an error.
    pub fn ended(&self, exit_code: i32, timed_out: bool) {
        self.span.record("exit_code", exit_code);
        self.span.record("timed_out", timed_out);
        if exit_code != 0 || timed_out {
            self.failed();
        }
    }

    /// The child could not be run (no jail, no gate): the span is an error.
    pub fn failed(&self) {
        self.span.record("otel.status_code", "error");
    }
}

/// Whether the subscriber d keeps the correlation attributes a child is told: one built on `Registry`, with `Correlate`,
/// the server's. The server checks it where it assembles its subscriber; on another, children are told no chain.
pub fn correlates(d: &tracing::Dispatch) -> bool {
    d.downcast_ref::<Registry>().is_some() && d.downcast_ref::<Correlate>().is_some()
}

/// What a child inside span is told of it: span's W3C trace context, none while the server's subscriber makes none, and
/// its correlation attributes, none when the subscriber keeps none (`correlates`).
fn trace_of(span: &Span) -> childenv::Trace {
    let sc = span.context().span().span_context().clone();
    let traceparent = sc
        .is_valid()
        .then(|| format!("00-{}-{}-{:02x}", sc.trace_id(), sc.span_id(), sc.trace_flags().to_u8()));
    let attributes = span
        .with_subscriber(|(id, dispatch)| {
            let data = dispatch.downcast_ref::<Registry>()?.span(id)?;
            let c = data.extensions().get::<Correlation>().cloned()?;
            Some(c.iter().map(|(name, value)| (name.to_owned(), value.text())).collect())
        })
        .flatten()
        .unwrap_or_default();
    childenv::Trace {
        traceparent,
        attributes,
    }
}

#[cfg(test)]
mod tests {
    use opentelemetry::trace::TracerProvider as _;
    use tracing_subscriber::layer::SubscriberExt as _;

    use super::*;

    #[test]
    fn a_child_is_told_its_span_and_the_chain_of_the_work_it_runs_for() {
        let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder().build();
        let otel = tracing_opentelemetry::layer().with_tracer(provider.tracer("test"));
        let subscriber = tracing_subscriber::registry().with(Correlate).with(otel);
        tracing::subscriber::with_default(subscriber, || {
            assert!(
                tracing::dispatcher::get_default(correlates),
                "the subscriber the server assembles"
            );
            let step = tracing::info_span!("remit.step", { attr::CHAIN_ID } = "ch_1");
            let _in = step.enter();
            let child = Child::open("check", "rl_1");
            let t = trace_of(child.span());
            let tp = t.traceparent.unwrap_or_default();
            let parts: Vec<&str> = tp.split('-').collect();
            assert!(
                matches!(parts[..], ["00", trace, span, "01"] if trace.len() == 32 && span.len() == 16),
                "a W3C traceparent: {tp:?}"
            );
            let sc = child.span().context().span().span_context().clone();
            assert_eq!(parts[2], sc.span_id().to_string(), "the child's own span");
            assert_eq!(
                parts[1],
                step.context().span().span_context().trace_id().to_string(),
                "in its work's trace"
            );
            assert_eq!(
                t.attributes,
                [
                    (attr::CHAIN_ID.to_owned(), "ch_1".to_owned()),
                    (attr::RELEASE_ID.to_owned(), "rl_1".to_owned())
                ]
            );
        });
    }
}
