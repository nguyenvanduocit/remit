//! A card's telemetry summed up for the owner: what its model requests cost and how fast they answered, which failed, and
//! how each tool ran. Pure: Claude Code's events in, the summary out.

use std::collections::BTreeMap;

use serde_json::{Map, Value};
use wire::app::{ApiError, CardTelemetry, ModelUse, ToolUse};

/// The events this summary reads: a reader of a card's events needs only these.
pub const SUMMARIZED: [&str; 3] = [API_REQUEST, API_ERROR, TOOL_RESULT];
const API_REQUEST: &str = "claude_code.api_request";
const API_ERROR: &str = "claude_code.api_error";
/// A tool's run: its name, how long it took and whether it succeeded.
pub const TOOL_RESULT: &str = "claude_code.tool_result";
/// The newest errors kept.
const RECENT_ERRORS: usize = 5;

/// Sums up a card's events (name, when in nanoseconds since the Unix epoch, attributes), in any order.
pub fn summarize(events: &[(String, i64, Map<String, Value>)]) -> CardTelemetry {
    let mut t = CardTelemetry::default();
    let (mut latencies, mut ttfts) = (Vec::new(), Vec::new());
    let mut models: BTreeMap<(String, String), ModelUse> = BTreeMap::new();
    let mut tools: BTreeMap<String, (ToolUse, Vec<u64>)> = BTreeMap::new();
    let mut errors = Vec::new();
    for (name, at_ns, a) in events {
        let at = at_ns.div_euclid(1_000_000);
        if t.first_at_ms == 0 || at < t.first_at_ms {
            t.first_at_ms = at;
        }
        t.last_at_ms = t.last_at_ms.max(at);
        match name.as_str() {
            API_REQUEST => {
                t.requests = t.requests.saturating_add(1);
                t.input_tokens = t.input_tokens.saturating_add(int(a, "input_tokens"));
                t.output_tokens = t.output_tokens.saturating_add(int(a, "output_tokens"));
                t.cache_read_tokens = t.cache_read_tokens.saturating_add(int(a, "cache_read_tokens"));
                t.cache_creation_tokens = t.cache_creation_tokens.saturating_add(int(a, "cache_creation_tokens"));
                let cost = float(a, "cost_usd");
                t.cost_usd += cost;
                latencies.push(int(a, "duration_ms"));
                if a.contains_key("ttft_ms") {
                    ttfts.push(int(a, "ttft_ms"));
                }
                let (model, effort) = (text(a, "model"), text(a, "effort"));
                let m = models
                    .entry((model.clone(), effort.clone()))
                    .or_insert_with(|| ModelUse {
                        model,
                        effort,
                        ..ModelUse::default()
                    });
                m.requests = m.requests.saturating_add(1);
                m.cost_usd += cost;
            }
            API_ERROR => {
                t.errors = t.errors.saturating_add(1);
                let status = match text(a, "status_code") {
                    s if s.is_empty() => text(a, "status"),
                    s => s,
                };
                errors.push(ApiError {
                    at_ms: at,
                    model: text(a, "model"),
                    status,
                    error: text(a, "error"),
                });
            }
            TOOL_RESULT => {
                let name = text(a, "tool_name");
                let (u, times) = tools.entry(name.clone()).or_insert_with(|| {
                    (
                        ToolUse {
                            name,
                            ..ToolUse::default()
                        },
                        Vec::new(),
                    )
                });
                let ms = int(a, "duration_ms");
                u.runs = u.runs.saturating_add(1);
                u.total_ms = u.total_ms.saturating_add(ms);
                if !truthy(a, "success") {
                    u.failed = u.failed.saturating_add(1);
                }
                times.push(ms);
            }
            _ => {}
        }
    }
    t.latency_p50_ms = percentile(&mut latencies, 50);
    t.latency_p95_ms = percentile(&mut latencies, 95);
    t.ttft_p50_ms = percentile(&mut ttfts, 50);
    t.models = models.into_values().collect();
    t.models.sort_by_key(|m| std::cmp::Reverse(m.requests));
    t.tools = tools
        .into_values()
        .map(|(mut u, mut times)| {
            u.p95_ms = percentile(&mut times, 95);
            u
        })
        .collect();
    t.tools.sort_by_key(|u| std::cmp::Reverse(u.total_ms));
    errors.sort_by_key(|e| std::cmp::Reverse(e.at_ms));
    errors.truncate(RECENT_ERRORS);
    t.recent_errors = errors;
    t
}

/// How many of events are tool results that failed of a tool whose name begins with one of prefixes: how often an
/// agent's call of one server's tools failed (ADR-0087 counts Remit's own).
pub fn failed_tools(events: &[(String, i64, Map<String, Value>)], prefixes: &[&str]) -> u64 {
    let failed = events.iter().filter(|(name, _, a)| {
        name == TOOL_RESULT && !truthy(a, "success") && prefixes.iter().any(|p| text(a, "tool_name").starts_with(p))
    });
    u64::try_from(failed.count()).unwrap_or(u64::MAX)
}

/// The nearest-rank percentile `p` of xs; zero for none.
fn percentile(xs: &mut [u64], p: usize) -> u64 {
    xs.sort_unstable();
    let rank = xs.len().saturating_mul(p).div_ceil(100).saturating_sub(1);
    xs.get(rank).copied().unwrap_or_default()
}

/// A number Claude Code sends as a number or as text ("158").
fn int(a: &Map<String, Value>, key: &str) -> u64 {
    match a.get(key) {
        Some(Value::Number(n)) => n.as_u64().unwrap_or_default(),
        Some(Value::String(s)) => s.parse().unwrap_or_default(),
        _ => 0,
    }
}

fn float(a: &Map<String, Value>, key: &str) -> f64 {
    match a.get(key) {
        Some(Value::Number(n)) => n.as_f64().unwrap_or_default(),
        Some(Value::String(s)) => s.parse().unwrap_or_default(),
        _ => 0.0,
    }
}

fn text(a: &Map<String, Value>, key: &str) -> String {
    match a.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(v) => v.to_string(),
    }
}

/// A flag Claude Code sends as a bool or as text ("true").
fn truthy(a: &Map<String, Value>, key: &str) -> bool {
    match a.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s == "true",
        _ => false,
    }
}
