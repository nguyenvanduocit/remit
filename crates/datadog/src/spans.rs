//! The server's spans, as the OpenTelemetry SDK ends them, put on the queue: the SDK's own batching would drop the newest
//! when full, and the queue drops the oldest.

use std::sync::Arc;
use std::time::Duration;

use opentelemetry::Context;
use opentelemetry_proto::tonic::common::v1::InstrumentationScope;
use opentelemetry_proto::tonic::resource::v1::Resource;
use opentelemetry_proto::tonic::trace::v1::Span;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::trace::{SpanData, SpanProcessor};

use crate::Inner;
use crate::queue::{Item, Record};

#[derive(Debug)]
pub(crate) struct Queued(pub(crate) Arc<Inner>);

impl Queued {
    fn resource(&self) -> Arc<Resource> {
        Arc::clone(&self.0.server)
    }

    fn scope(&self) -> Arc<InstrumentationScope> {
        Arc::clone(&self.0.scope)
    }
}

impl SpanProcessor for Queued {
    fn on_start(&self, _span: &mut opentelemetry_sdk::trace::Span, _cx: &Context) {}

    fn on_end(&self, span: SpanData) {
        if !self.0.wanted() {
            return;
        }
        self.0.queue.put([Item {
            resource: self.resource(),
            scope: self.scope(),
            record: Record::Span(Span::from(span)),
        }]);
    }

    /// The queue is the exporter's to send: there is nothing held here.
    fn force_flush(&self) -> OTelSdkResult {
        Ok(())
    }

    fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
        Ok(())
    }
}
