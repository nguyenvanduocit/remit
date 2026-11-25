//! What leaves for Datadog carries no credential, by the sources' own rules (`source::redact`): the value of every
//! attribute whose key says it holds one is removed (`sensitive_key`, as `redact_value` does for a field), and every text
//! (a log's body, an attribute's string, a span's status) is cleaned of token shapes and addresses (`clean_text`, as
//! `clean_texts` does). A record is redacted as the exporter lays it out, before it is measured, off the path of the work
//! that logged it; the resource and scope an agent's session brings, as the door takes them. The server's own resource
//! and scope are Remit's tags only.

use opentelemetry_proto::tonic::common::v1::{AnyValue, InstrumentationScope, KeyValue, any_value};
use opentelemetry_proto::tonic::resource::v1::Resource;

use crate::forward::points;
use crate::queue::Record;

pub(crate) fn record(r: &mut Record) {
    match r {
        Record::Log(l) => {
            if let Some(body) = l.body.as_mut() {
                value(body);
            }
            attributes(&mut l.attributes);
        }
        Record::Span(s) => {
            attributes(&mut s.attributes);
            for e in &mut s.events {
                attributes(&mut e.attributes);
            }
            for l in &mut s.links {
                attributes(&mut l.attributes);
            }
            if let Some(st) = s.status.as_mut() {
                st.message = source::clean_text(&st.message);
            }
        }
        Record::Metric(m) => {
            for (attrs, _) in points(m) {
                attributes(attrs);
            }
        }
    }
}

pub(crate) fn resource(r: &mut Resource) {
    attributes(&mut r.attributes);
}

pub(crate) fn scope(s: &mut InstrumentationScope) {
    attributes(&mut s.attributes);
}

fn attributes(attrs: &mut [KeyValue]) {
    for kv in attrs {
        let Some(v) = kv.value.as_mut() else { continue };
        if source::sensitive_key(&kv.key) && !empty(v) {
            v.value = Some(any_value::Value::StringValue(source::REMOVED.to_owned()));
        } else {
            value(v);
        }
    }
}

fn value(v: &mut AnyValue) {
    match v.value.as_mut() {
        Some(any_value::Value::StringValue(s)) => *s = source::clean_text(s),
        Some(any_value::Value::ArrayValue(a)) => a.values.iter_mut().for_each(value),
        Some(any_value::Value::KvlistValue(kv)) => attributes(&mut kv.values),
        _ => {}
    }
}

/// That a credential is missing is something an investigation needs to see, as `redact_value` keeps it.
fn empty(v: &AnyValue) -> bool {
    match &v.value {
        None => true,
        Some(any_value::Value::StringValue(s)) => s.is_empty(),
        _ => false,
    }
}
