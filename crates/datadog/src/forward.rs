//! What an agent's session exports to the `/otlp` door, made Remit's before it leaves: every attribute the payload brings
//! under `remit`, at any depth (Datadog flattens a map `remit: {chain_id}` to `remit.chain_id`), is dropped and the
//! card's own set from its run's token (`Stamp`), so a payload cannot claim another chain. Each trace is placed once
//! (`Turns::place`), by its earliest span: in the turn of its card it began in, its root spans the children of the turn's
//! span; in no turn, in a trace of the card's own. Every span and log of one trace goes where it went, so a session's work
//! reads inside the turn that asked for it whatever the session propagates, and a payload's ids never reach into a trace
//! that is not its card's. Pure but for `Datadog::forward`, which queues what this makes.

use std::collections::HashMap;
use std::sync::Arc;

use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::common::v1::{InstrumentationScope, KeyValue, any_value};
use opentelemetry_proto::tonic::logs::v1::LogRecord;
use opentelemetry_proto::tonic::metrics::v1::{Metric, metric};
use opentelemetry_proto::tonic::resource::v1::Resource;
use opentelemetry_proto::tonic::trace::v1::Span;
use telemetry::attr;

use crate::queue::{Item, Record};
use crate::turns::Placed;
use crate::{Datadog, SERVICE_AGENT, Signal, Turn, Turns, int, redact, resource, text};

/// Whose a session's records are, as its run's token names it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stamp {
    pub chain_id: String,
    pub card_id: String,
}

/// Why a body was not taken: it is not the OTLP JSON request of its signal.
#[derive(Debug, thiserror::Error)]
#[error("not an OTLP JSON {signal:?} request: {source}")]
pub struct Refused {
    signal: Signal,
    source: serde_json::Error,
}

impl Datadog {
    /// Queues what body, an OTLP JSON request of signal from who's session, carries, stamped and put under its turns;
    /// nothing while nothing is sent.
    pub fn forward(&self, signal: Signal, body: &[u8], who: &Stamp) -> Result<(), Refused> {
        let i = &self.inner;
        if !i.wanted() {
            return Ok(());
        }
        let refused = |source| Refused { signal, source };
        let made = Made {
            who,
            env: &i.env,
            version: &i.version,
            turns: &i.turns,
        };
        let items = match signal {
            Signal::Logs => made.logs(serde_json::from_slice(body).map_err(refused)?),
            Signal::Metrics => made.metrics(serde_json::from_slice(body).map_err(refused)?),
            Signal::Traces => made.traces(serde_json::from_slice(body).map_err(refused)?),
        };
        i.queue.put(items);
        Ok(())
    }
}

/// What makes a session's records Remit's: whose they are, the tags every record carries, and the card's turns.
struct Made<'a> {
    who: &'a Stamp,
    env: &'a str,
    version: &'a str,
    turns: &'a Turns,
}

impl Made<'_> {
    /// The resource of a session: its own attributes but Remit's, then the agent's service tags and whose it is.
    fn resource(&self, r: Option<Resource>) -> Arc<Resource> {
        let mut own = r.unwrap_or_default();
        let tags = resource(SERVICE_AGENT, self.env, self.version).attributes;
        unclaimed(&mut own.attributes);
        own.attributes.retain(|kv| !tags.iter().any(|t| t.key == kv.key));
        redact::resource(&mut own);
        own.attributes.extend(tags);
        own.attributes.extend(self.whose());
        Arc::new(own)
    }

    fn scope(s: Option<InstrumentationScope>) -> Arc<InstrumentationScope> {
        let mut s = s.unwrap_or_default();
        unclaimed(&mut s.attributes);
        redact::scope(&mut s);
        Arc::new(s)
    }

    /// The attributes that say whose a record is: on the record too, as Datadog does not make every resource attribute
    /// a tag of every signal.
    fn whose(&self) -> [KeyValue; 2] {
        [
            text(attr::CHAIN_ID, &self.who.chain_id),
            text(attr::CARD_ID, &self.who.card_id),
        ]
    }

    /// attrs as Remit's: whose they are and the turn, in place of anything under `remit` they brought.
    fn stamp(&self, attrs: &mut Vec<KeyValue>, turn: Option<Turn>) {
        unclaimed(attrs);
        attrs.extend(self.whose());
        if let Some(t) = turn {
            attrs.push(int(attr::TURN, t.turn));
        }
    }

    fn place(&self, trace: &[u8], earliest_ns: u64) -> Placed {
        self.turns.place(&self.who.card_id, trace, earliest_ns)
    }

    fn logs(&self, req: ExportLogsServiceRequest) -> Vec<Item> {
        let mut out = Vec::new();
        for rl in req.resource_logs {
            let resource = self.resource(rl.resource);
            for sl in rl.scope_logs {
                let scope = Self::scope(sl.scope);
                for mut r in sl.log_records {
                    self.log(&mut r);
                    out.push(Item {
                        resource: Arc::clone(&resource),
                        scope: Arc::clone(&scope),
                        record: Record::Log(r),
                    });
                }
            }
        }
        out
    }

    /// A log record where its trace went, or with none in the turn of its time: under the session's span it names, or
    /// else the turn's own.
    fn log(&self, r: &mut LogRecord) {
        let at = if r.time_unix_nano > 0 {
            r.time_unix_nano
        } else {
            r.observed_time_unix_nano
        };
        let turn = if r.trace_id.is_empty() {
            let turn = self.turns.at(&self.who.card_id, at);
            if let Some(t) = turn {
                r.trace_id = t.trace_id.to_vec();
            }
            turn
        } else {
            let p = self.place(&r.trace_id, at);
            r.trace_id = p.trace_id.to_vec();
            p.turn
        };
        if let Some(t) = turn
            && r.span_id.is_empty()
        {
            r.span_id = t.span_id.to_vec();
        }
        self.stamp(&mut r.attributes, turn);
    }

    fn traces(&self, req: ExportTraceServiceRequest) -> Vec<Item> {
        let mut earliest: HashMap<Vec<u8>, u64> = HashMap::new();
        for s in req
            .resource_spans
            .iter()
            .flat_map(|r| &r.scope_spans)
            .flat_map(|s| &s.spans)
        {
            let e = earliest.entry(s.trace_id.clone()).or_insert(u64::MAX);
            *e = (*e).min(s.start_time_unix_nano);
        }
        let placed: HashMap<Vec<u8>, Placed> = earliest
            .into_iter()
            .map(|(trace, at)| {
                let p = self.place(&trace, at);
                (trace, p)
            })
            .collect();
        let mut out = Vec::new();
        for rs in req.resource_spans {
            let resource = self.resource(rs.resource);
            for ss in rs.scope_spans {
                let scope = Self::scope(ss.scope);
                for mut s in ss.spans {
                    if let Some(p) = placed.get(&s.trace_id) {
                        self.span(&mut s, *p);
                    }
                    out.push(Item {
                        resource: Arc::clone(&resource),
                        scope: Arc::clone(&scope),
                        record: Record::Span(s),
                    });
                }
            }
        }
        out
    }

    /// A span where its trace went: a root span the child of its turn's span.
    fn span(&self, s: &mut Span, p: Placed) {
        s.trace_id = p.trace_id.to_vec();
        if let Some(t) = p.turn
            && s.parent_span_id.is_empty()
        {
            s.parent_span_id = t.span_id.to_vec();
        }
        self.stamp(&mut s.attributes, p.turn);
    }

    fn metrics(&self, req: ExportMetricsServiceRequest) -> Vec<Item> {
        let mut out = Vec::new();
        for rm in req.resource_metrics {
            let resource = self.resource(rm.resource);
            for sm in rm.scope_metrics {
                let scope = Self::scope(sm.scope);
                for mut m in sm.metrics {
                    for (attrs, at) in points(&mut m) {
                        self.stamp(attrs, self.turns.at(&self.who.card_id, at));
                    }
                    out.push(Item {
                        resource: Arc::clone(&resource),
                        scope: Arc::clone(&scope),
                        record: Record::Metric(m),
                    });
                }
            }
        }
        out
    }
}

/// attrs without anything under `remit`, at any depth: only Remit says whose a record is.
fn unclaimed(attrs: &mut Vec<KeyValue>) {
    attrs.retain(|kv| kv.key.split('.').next() != Some(attr::PREFIX.trim_end_matches('.')));
    for kv in attrs {
        if let Some(v) = kv.value.as_mut() {
            unclaimed_in(v);
        }
    }
}

fn unclaimed_in(v: &mut opentelemetry_proto::tonic::common::v1::AnyValue) {
    match v.value.as_mut() {
        Some(any_value::Value::KvlistValue(kv)) => unclaimed(&mut kv.values),
        Some(any_value::Value::ArrayValue(a)) => a.values.iter_mut().for_each(unclaimed_in),
        _ => {}
    }
}

/// The attributes of each data point of m, with its time.
pub(crate) fn points(m: &mut Metric) -> Vec<(&mut Vec<KeyValue>, u64)> {
    match &mut m.data {
        Some(metric::Data::Gauge(g)) => g
            .data_points
            .iter_mut()
            .map(|p| (&mut p.attributes, p.time_unix_nano))
            .collect(),
        Some(metric::Data::Sum(s)) => s
            .data_points
            .iter_mut()
            .map(|p| (&mut p.attributes, p.time_unix_nano))
            .collect(),
        Some(metric::Data::Histogram(h)) => h
            .data_points
            .iter_mut()
            .map(|p| (&mut p.attributes, p.time_unix_nano))
            .collect(),
        Some(metric::Data::ExponentialHistogram(h)) => h
            .data_points
            .iter_mut()
            .map(|p| (&mut p.attributes, p.time_unix_nano))
            .collect(),
        Some(metric::Data::Summary(s)) => s
            .data_points
            .iter_mut()
            .map(|p| (&mut p.attributes, p.time_unix_nano))
            .collect(),
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TURN_TRACE: [u8; 16] = [7; 16];
    const TURN_SPAN: [u8; 8] = [9; 8];
    const NEXT_TRACE: [u8; 16] = [8; 16];

    fn who() -> Stamp {
        Stamp {
            chain_id: "ch_mine".into(),
            card_id: "k_mine".into(),
        }
    }

    fn made<'a>(who: &'a Stamp, turns: &'a Turns) -> Made<'a> {
        Made {
            who,
            env: "test",
            version: "v1",
            turns,
        }
    }

    /// k_mine's turn 42 began at 1000, its turn 43 at 2050.
    fn turns() -> Turns {
        let t = Turns::default();
        t.began(
            "k_mine",
            Turn {
                start_ns: 1000,
                turn: 42,
                trace_id: TURN_TRACE,
                span_id: TURN_SPAN,
            },
        );
        t.began(
            "k_mine",
            Turn {
                start_ns: 2050,
                turn: 43,
                trace_id: NEXT_TRACE,
                span_id: [10; 8],
            },
        );
        t
    }

    fn value<'a>(attrs: &'a [KeyValue], key: &str) -> Vec<&'a any_value::Value> {
        attrs
            .iter()
            .filter(|kv| kv.key == key)
            .filter_map(|kv| kv.value.as_ref()?.value.as_ref())
            .collect()
    }

    fn texts<'a>(attrs: &'a [KeyValue], key: &str) -> Vec<&'a str> {
        value(attrs, key)
            .into_iter()
            .filter_map(|v| match v {
                any_value::Value::StringValue(s) => Some(s.as_str()),
                _ => None,
            })
            .collect()
    }

    fn spans(items: &[Item]) -> Vec<&Span> {
        items
            .iter()
            .filter_map(|it| match &it.record {
                Record::Span(s) => Some(s),
                _ => None,
            })
            .collect()
    }

    /// A session's export, as Claude Code writes it: its own trace, a root span and its child, the child begun after the
    /// card's next turn did, and a payload that claims another chain on its resource and its spans, flat and as a map
    /// Datadog would flatten.
    const TRACES: &str = r#"{"resourceSpans":[{"resource":{"attributes":[
        {"key":"service.name","value":{"stringValue":"claude-code"}},
        {"key":"remit.chain_id","value":{"stringValue":"ch_other"}},
        {"key":"remit","value":{"kvlistValue":{"values":[{"key":"chain_id","value":{"stringValue":"ch_other"}}]}}}]},
      "scopeSpans":[{"scope":{"name":"com.anthropic.claude_code"},"spans":[
        {"traceId":"0102030405060708090a0b0c0d0e0f10","spanId":"1111111111111111","name":"claude_code.interaction",
         "startTimeUnixNano":2000,"endTimeUnixNano":3000,
         "attributes":[{"key":"remit.card_id","value":{"stringValue":"k_other"}},{"key":"n","value":{"intValue":3}},
           {"key":"tool","value":{"kvlistValue":{"values":[
             {"key":"remit","value":{"kvlistValue":{"values":[{"key":"chain_id","value":{"stringValue":"ch_other"}}]}}}]}}}]},
        {"traceId":"0102030405060708090a0b0c0d0e0f10","spanId":"2222222222222222","parentSpanId":"1111111111111111",
         "name":"claude_code.tool","startTimeUnixNano":2100,"endTimeUnixNano":2900}]}]}]}"#;

    #[test]
    fn a_sessions_trace_joins_the_turn_it_began_in_whole_and_names_only_the_token_s_chain() {
        let (who, turns) = (who(), turns());
        let items = made(&who, &turns).traces(serde_json::from_str(TRACES).unwrap());
        let got = spans(&items);
        let (root, child) = (got[0], got[1]);
        assert_eq!(root.trace_id, TURN_TRACE.to_vec());
        assert_eq!(root.parent_span_id, TURN_SPAN.to_vec(), "the root goes under the turn");
        assert_eq!(
            child.trace_id,
            TURN_TRACE.to_vec(),
            "begun after turn 43 did, it stays in its trace's turn"
        );
        assert_eq!(child.parent_span_id, vec![0x11; 8], "a child keeps its parent");
        for s in [root, child] {
            assert_eq!(texts(&s.attributes, attr::CHAIN_ID), ["ch_mine"]);
            assert_eq!(texts(&s.attributes, attr::CARD_ID), ["k_mine"]);
            assert_eq!(value(&s.attributes, attr::TURN), [&any_value::Value::IntValue(42)]);
        }
        let tool = value(&root.attributes, "tool");
        assert!(
            matches!(tool[..], [any_value::Value::KvlistValue(kv)] if kv.values.is_empty()),
            "a nested claim is gone: {tool:?}"
        );
        let res = &items[0].resource.attributes;
        assert_eq!(texts(res, attr::CHAIN_ID), ["ch_mine"], "the payload's claim is gone");
        assert!(value(res, "remit").is_empty(), "and the one Datadog would flatten");
        assert_eq!(texts(res, "service.name"), [SERVICE_AGENT]);

        // the rest of the trace, in a later export, goes where its trace went
        let later = TRACES
            .replace("2222222222222222", "3333333333333333")
            .replace("2100", "2500");
        let again = made(&who, &turns).traces(serde_json::from_str(&later).unwrap());
        assert!(spans(&again).iter().all(|s| s.trace_id == TURN_TRACE.to_vec()));
    }

    #[test]
    fn a_trace_in_no_turn_goes_in_a_trace_of_the_card_s_own_and_still_names_its_chain() {
        let (who, turns) = (who(), Turns::default());
        let items = made(&who, &turns).traces(serde_json::from_str(TRACES).unwrap());
        let got = spans(&items);
        let root = got[0];
        assert_ne!(
            root.trace_id[0], 1,
            "the payload's trace is not kept: it could be another card's"
        );
        assert!(got.iter().all(|s| s.trace_id == root.trace_id), "one trace stays one");
        assert!(root.parent_span_id.is_empty());
        assert_eq!(texts(&root.attributes, attr::CHAIN_ID), ["ch_mine"]);
        assert_eq!(texts(&root.attributes, attr::CARD_ID), ["k_mine"]);

        let other = Stamp {
            chain_id: "ch_x".into(),
            card_id: "k_x".into(),
        };
        let theirs = made(&other, &turns).traces(serde_json::from_str(TRACES).unwrap());
        assert_ne!(
            spans(&theirs)[0].trace_id,
            root.trace_id,
            "the same ids from another card part"
        );
    }

    #[test]
    fn a_log_with_no_span_is_put_on_its_turn_s_span() {
        let (who, turns) = (who(), turns());
        let body = r#"{"resourceLogs":[{"scopeLogs":[{"logRecords":[{"timeUnixNano":"1500","body":{"stringValue":"claude_code.api_request"},
            "attributes":[{"key":"remit.chain_id","value":{"stringValue":"ch_other"}}]}]}]}]}"#;
        let items = made(&who, &turns).logs(serde_json::from_str(body).unwrap());
        let Some(Record::Log(r)) = items.first().map(|it| &it.record) else {
            panic!("no log")
        };
        assert_eq!(r.trace_id, TURN_TRACE.to_vec());
        assert_eq!(r.span_id, TURN_SPAN.to_vec());
        assert_eq!(texts(&r.attributes, attr::CHAIN_ID), ["ch_mine"]);
    }
}
