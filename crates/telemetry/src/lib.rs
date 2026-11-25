//! What the agents' Claude Code sessions export as OpenTelemetry (ADR-0085), as wide events: one record per log record,
//! metric data point or span, with every attribute in one JSON object, so a new attribute or event needs no change here.
//! Pure: `otlp` reads a request's body into records, `summarize` sums a card's events up for the owner; the store keeps
//! them (`store::Telemetry`). The server's spans carry the correlation attributes (`correlate`), which a child it starts
//! is told with its own span (`child`).

pub mod attr;
mod child;
pub mod correlate;
pub mod otlp;
mod summary;

use serde_json::{Map, Value};

pub use child::{Child, correlates};
pub use correlate::{Correlate, Correlation};
pub use summary::{SUMMARIZED, TOOL_RESULT, failed_tools, summarize};

/// What a record is in OpenTelemetry's model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A log record: Claude Code's events (`claude_code.api_request`, `claude_code.tool_result`, ...).
    Event,
    /// A metric's data point.
    Metric,
    Span,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Event => "event",
            Self::Metric => "metric",
            Self::Span => "span",
        }
    }
}

/// One wide event. `value` is a metric's number or a span's duration in milliseconds; `parent_id` a span's parent.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    /// When it happened, in nanoseconds since the Unix epoch (a span: when it started).
    pub at_ns: i64,
    pub kind: Kind,
    pub name: String,
    pub value: Option<f64>,
    pub trace_id: String,
    pub span_id: String,
    pub parent_id: String,
    pub attrs: Map<String, Value>,
}
