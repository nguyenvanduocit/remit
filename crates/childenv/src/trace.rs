//! What a child is told of the work it runs for (ADR-0119): the span Remit opened for it, as W3C trace context, and the
//! correlation attributes of that work, added to its resource, so whatever the child reports or passes on (a request to a
//! product it deploys, a log it prints) leads back to the chain. Its service stays its own: a check or a release step may
//! run the customer's product, whose telemetry keeps its name. These names carry ids, never a credential: the Datadog key
//! stays in the server, and no endpoint or token to export to is given here (a way for a child to export to Remit's
//! `/otlp` door is the ADR's *Revisit when*).

/// The variable a child's resource attributes are in, as OpenTelemetry's SDKs read it.
const RESOURCE: &str = "OTEL_RESOURCE_ATTRIBUTES";
/// The variable a child's parent span is in (W3C trace context, OpenTelemetry's environment propagator).
const PARENT: &str = "TRACEPARENT";

/// The work a child runs for, as Remit's span of it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Trace {
    /// The child's span, `00-<trace id>-<span id>-<flags>`; none when the server records no trace.
    pub traceparent: Option<String>,
    /// The correlation attributes of the work (`remit.chain_id` and those known with it), by name.
    pub attributes: Vec<(String, String)>,
}

/// env, a child's `NAME=value` entries, told of t: `TRACEPARENT` set to t's span in place of any env held, and t's
/// attributes appended to the `OTEL_RESOURCE_ATTRIBUTES` env holds, after its own, so a name of Remit's is the one an SDK
/// reads (the last of a name wins) and the job's own attributes stay. Each name and value Remit adds is escaped, as the
/// specification asks, so a value cannot add an attribute of its own.
pub fn trace(env: Vec<String>, t: &Trace) -> Vec<String> {
    let ours: Vec<String> = t
        .attributes
        .iter()
        .map(|(name, value)| format!("{}={}", urlescape::component(name), urlescape::component(value)))
        .collect();
    let mut own_resource = String::new();
    let mut out: Vec<String> = env
        .into_iter()
        .filter(|kv| match kv.split_once('=') {
            Some((RESOURCE, value)) => {
                value.clone_into(&mut own_resource);
                false
            }
            Some((PARENT, _)) => t.traceparent.is_none(),
            _ => true,
        })
        .collect();
    if let Some(tp) = &t.traceparent {
        out.push(format!("{PARENT}={tp}"));
    }
    let resource: Vec<&str> = std::iter::once(own_resource.as_str())
        .filter(|own| !own.is_empty())
        .chain(ours.iter().map(String::as_str))
        .collect();
    if !resource.is_empty() {
        out.push(format!("{RESOURCE}={}", resource.join(",")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_cannot_add_an_attribute_of_its_own() {
        let t = Trace {
            traceparent: Some("00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01".into()),
            attributes: vec![
                ("remit.chain_id".into(), "ch_1".into()),
                ("remit.card_id".into(), "a,remit.chain_id=ch_other b".into()),
            ],
        };
        let env = vec![
            "PATH=/bin".to_owned(),
            "OTEL_RESOURCE_ATTRIBUTES=deployment.environment=prod".to_owned(),
            "TRACEPARENT=00-ffffffffffffffffffffffffffffffff-ffffffffffffffff-01".to_owned(),
        ];
        assert_eq!(
            trace(env, &t),
            [
                "PATH=/bin",
                "TRACEPARENT=00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
                "OTEL_RESOURCE_ATTRIBUTES=deployment.environment=prod,remit.chain_id=ch_1,\
                 remit.card_id=a%2Cremit.chain_id%3Dch_other%20b",
            ]
        );
    }
}
