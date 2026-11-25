//! What waits to leave for Datadog: one record at a time, each with the resource and scope it came under, in a queue
//! bounded in records and in bytes that drops its oldest when full and counts what it dropped. Putting never blocks or waits: the work that
//! logs goes on whatever Datadog does. The exporter takes it all at once and lays it out in requests under the intake's
//! limits (`requests`), each record redacted first (`redact`).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::common::v1::InstrumentationScope;
use opentelemetry_proto::tonic::logs::v1::{LogRecord, ResourceLogs, ScopeLogs};
use opentelemetry_proto::tonic::metrics::v1::{Metric, ResourceMetrics, ScopeMetrics};
use opentelemetry_proto::tonic::resource::v1::Resource;
use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span};
use prost::Message;
use tokio::sync::Notify;

use crate::redact;
use crate::site::Signal;

/// One record of any signal.
#[derive(Debug, PartialEq)]
pub(crate) enum Record {
    Log(LogRecord),
    Metric(Metric),
    Span(Span),
}

impl Record {
    pub(crate) fn signal(&self) -> Signal {
        match self {
            Record::Log(_) => Signal::Logs,
            Record::Metric(_) => Signal::Metrics,
            Record::Span(_) => Signal::Traces,
        }
    }

    pub(crate) fn encoded_len(&self) -> usize {
        match self {
            Record::Log(r) => r.encoded_len(),
            Record::Metric(m) => m.encoded_len(),
            Record::Span(s) => s.encoded_len(),
        }
    }
}

/// A record with what it came under; records of one resource and scope share them.
#[derive(Debug)]
pub(crate) struct Item {
    pub resource: Arc<Resource>,
    pub scope: Arc<InstrumentationScope>,
    pub record: Record,
}

/// How many records wait at most, and how many bytes they take: past either the oldest go.
const CAPACITY: usize = 50_000;
const CAPACITY_BYTES: usize = 64 << 20;

/// The records waiting, each with its size, and their bytes.
#[derive(Debug, Default)]
struct Waiting {
    items: VecDeque<(usize, Item)>,
    bytes: usize,
}

/// The records waiting, and how many were dropped since the start.
#[derive(Debug, Default)]
pub(crate) struct Queue {
    waiting: Mutex<Waiting>,
    dropped: AtomicU64,
    /// Woken at every put, so the exporter sends what came soon after it came.
    pub(crate) put: Notify,
}

impl Queue {
    /// Puts items at the back, dropping the oldest past `CAPACITY` or `CAPACITY_BYTES`.
    pub(crate) fn put(&self, items: impl IntoIterator<Item = Item>) {
        let sized: Vec<_> = items.into_iter().map(|it| (it.record.encoded_len(), it)).collect();
        let mut q = self.waiting.lock().unwrap_or_else(PoisonError::into_inner);
        let mut over = 0u64;
        for (len, it) in sized {
            while !q.items.is_empty() && (q.items.len() >= CAPACITY || q.bytes.saturating_add(len) > CAPACITY_BYTES) {
                if let Some((gone, _)) = q.items.pop_front() {
                    q.bytes = q.bytes.saturating_sub(gone);
                    over = over.saturating_add(1);
                }
            }
            q.bytes = q.bytes.saturating_add(len);
            q.items.push_back((len, it));
        }
        drop(q);
        if over > 0 {
            self.dropped.fetch_add(over, Ordering::Relaxed);
        }
        self.put.notify_one();
    }

    /// Everything waiting, oldest first.
    pub(crate) fn take(&self) -> Vec<Item> {
        let mut q = self.waiting.lock().unwrap_or_else(PoisonError::into_inner);
        q.bytes = 0;
        q.items.drain(..).map(|(_, it)| it).collect()
    }

    /// How many records were dropped since the start.
    pub(crate) fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

/// items laid out as requests, the body of each encoded, in the order they came: records of one signal, one resource and
/// one scope that follow each other share their entries, and a request is cut before it would pass its signal's limit.
/// Each record is redacted before it is measured.
pub(crate) fn requests(items: Vec<Item>) -> Vec<(Signal, Vec<u8>)> {
    let mut out = Vec::new();
    let mut rest = items;
    for signal in Signal::ALL {
        let (mine, others): (Vec<Item>, Vec<Item>) = rest.into_iter().partition(|it| it.record.signal() == signal);
        rest = others;
        let mut b = Builder::new(signal);
        for mut it in mine {
            redact::record(&mut it.record);
            if let Some(body) = b.add(it) {
                out.push((signal, body));
            }
        }
        if let Some(body) = b.finish() {
            out.push((signal, body));
        }
    }
    out
}

/// The records of one scope.
type Scoped = (Arc<InstrumentationScope>, Vec<Record>);

/// The most a message's framing inside its parent takes: its field's tag and its length, under 4 GiB. Counted for each
/// record, scope and resource, it makes a request's measure an upper bound of its encoded size.
const FRAMING: usize = 6;

/// One request of one signal as it is made.
struct Builder {
    signal: Signal,
    /// Each resource's entry, with its scopes, each with its records.
    groups: Vec<(Arc<Resource>, Vec<Scoped>)>,
    size: usize,
}

impl Builder {
    fn new(signal: Signal) -> Self {
        Self {
            signal,
            groups: Vec::new(),
            size: 0,
        }
    }

    /// Adds it; the request made so far when it would pass the limit, it then the first record of the next one.
    fn add(&mut self, it: Item) -> Option<Vec<u8>> {
        // a new resource and scope may come with it: counted as if they did, so a request stays under its limit
        let len = it
            .record
            .encoded_len()
            .saturating_add(FRAMING)
            .saturating_add(it.resource.encoded_len().saturating_add(2 * FRAMING))
            .saturating_add(it.scope.encoded_len().saturating_add(2 * FRAMING));
        let full = !self.groups.is_empty() && self.size.saturating_add(len) > self.signal.request_limit();
        let done = if full { self.finish() } else { None };
        self.size = self.size.saturating_add(len);
        let same_resource = self.groups.last().is_some_and(|(r, _)| Arc::ptr_eq(r, &it.resource));
        if !same_resource {
            self.groups.push((it.resource, Vec::new()));
        }
        if let Some((_, scopes)) = self.groups.last_mut() {
            let same_scope = scopes.last().is_some_and(|(s, _)| Arc::ptr_eq(s, &it.scope));
            if !same_scope {
                scopes.push((it.scope, Vec::new()));
            }
            if let Some((_, records)) = scopes.last_mut() {
                records.push(it.record);
            }
        }
        done
    }

    /// The request made so far, encoded; none when it holds nothing.
    fn finish(&mut self) -> Option<Vec<u8>> {
        let groups = std::mem::take(&mut self.groups);
        self.size = 0;
        if groups.is_empty() {
            return None;
        }
        let res = |r: Arc<Resource>| Some(Arc::unwrap_or_clone(r));
        let scope = |s: Arc<InstrumentationScope>| Some(Arc::unwrap_or_clone(s));
        Some(match self.signal {
            Signal::Logs => ExportLogsServiceRequest {
                resource_logs: groups
                    .into_iter()
                    .map(|(r, scopes)| ResourceLogs {
                        resource: res(r),
                        scope_logs: scopes
                            .into_iter()
                            .map(|(s, records)| ScopeLogs {
                                scope: scope(s),
                                log_records: records
                                    .into_iter()
                                    .filter_map(|x| match x {
                                        Record::Log(l) => Some(l),
                                        _ => None,
                                    })
                                    .collect(),
                                ..ScopeLogs::default()
                            })
                            .collect(),
                        ..ResourceLogs::default()
                    })
                    .collect(),
            }
            .encode_to_vec(),
            Signal::Metrics => ExportMetricsServiceRequest {
                resource_metrics: groups
                    .into_iter()
                    .map(|(r, scopes)| ResourceMetrics {
                        resource: res(r),
                        scope_metrics: scopes
                            .into_iter()
                            .map(|(s, records)| ScopeMetrics {
                                scope: scope(s),
                                metrics: records
                                    .into_iter()
                                    .filter_map(|x| match x {
                                        Record::Metric(m) => Some(m),
                                        _ => None,
                                    })
                                    .collect(),
                                ..ScopeMetrics::default()
                            })
                            .collect(),
                        ..ResourceMetrics::default()
                    })
                    .collect(),
            }
            .encode_to_vec(),
            Signal::Traces => ExportTraceServiceRequest {
                resource_spans: groups
                    .into_iter()
                    .map(|(r, scopes)| ResourceSpans {
                        resource: res(r),
                        scope_spans: scopes
                            .into_iter()
                            .map(|(s, records)| ScopeSpans {
                                scope: scope(s),
                                spans: records
                                    .into_iter()
                                    .filter_map(|x| match x {
                                        Record::Span(s) => Some(s),
                                        _ => None,
                                    })
                                    .collect(),
                                ..ScopeSpans::default()
                            })
                            .collect(),
                        ..ResourceSpans::default()
                    })
                    .collect(),
            }
            .encode_to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use opentelemetry_proto::tonic::common::v1::{AnyValue, any_value};

    use super::*;

    /// A log of about 300 KiB under one of two resources, as a busy session's tool output would make.
    fn big_logs(n: usize) -> Vec<Item> {
        let resources = [Arc::new(Resource::default()), Arc::new(Resource::default())];
        let scope = Arc::new(InstrumentationScope::default());
        (0..n)
            .map(|i| Item {
                resource: Arc::clone(&resources[i % 2]),
                scope: Arc::clone(&scope),
                record: Record::Log(LogRecord {
                    body: Some(AnyValue {
                        value: Some(any_value::Value::StringValue(format!("{i} {}", "x".repeat(300 << 10)))),
                    }),
                    ..LogRecord::default()
                }),
            })
            .collect()
    }

    #[test]
    fn requests_stay_under_the_intake_s_limit_and_carry_every_record() {
        let made = requests(big_logs(60));
        assert!(
            made.len() >= 4,
            "18 MiB of logs is cut in several requests: {}",
            made.len()
        );
        let mut carried = 0;
        for (signal, body) in &made {
            assert_eq!(*signal, Signal::Logs);
            assert!(
                body.len() <= 5_347_737,
                "a request of {} bytes passes 5.1 MiB",
                body.len()
            );
            let req = ExportLogsServiceRequest::decode(body.as_slice()).unwrap();
            carried += req
                .resource_logs
                .iter()
                .flat_map(|r| &r.scope_logs)
                .map(|s| s.log_records.len())
                .sum::<usize>();
        }
        assert_eq!(carried, 60);
    }
}
