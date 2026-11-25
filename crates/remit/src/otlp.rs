//! The door agents' Claude Code sessions export their OpenTelemetry through (ADR-0085): OTLP/HTTP JSON at
//! `/otlp/v1/logs`, `/otlp/v1/metrics` and `/otlp/v1/traces`, with the bearer token of a live run, as `/mcp` takes it. What
//! comes in is kept as its card's, the card named by the token, never by the payload; once kept it is also sent to
//! Datadog, stamped with that card's chain and put under the card's turn (ADR-0119, `datadog::Datadog::forward`), so a
//! session that retries what could not be kept does not send it twice.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use datadog::{Datadog, Signal, Stamp};
use orch::Orchestrator;
use store::Telemetry;
use telemetry::{Record, otlp};
use tokio_util::sync::CancellationToken;

use crate::agentmcp::{bearer, refusal};

/// Where the door is mounted (`orch::Config::otlp_url` names it).
pub const PATTERN: &str = "/otlp";
/// The most one request may carry.
const BODY_LIMIT: usize = 8_388_608;

struct Door {
    o: Arc<Orchestrator>,
    t: Telemetry,
    dd: Datadog,
}

type Decode = fn(&[u8]) -> Result<Vec<Record>, otlp::DecodeError>;

/// How long a record is kept.
const KEEP: Duration = Duration::from_secs(90 * 24 * 3600);
/// How often what is older than `KEEP` is removed.
const FORGET_EVERY: Duration = Duration::from_secs(24 * 3600);

/// Removes what is older than `KEEP`, now and every `FORGET_EVERY`, until `stop`.
pub async fn forget_old(t: Telemetry, stop: CancellationToken) {
    let mut every = tokio::time::interval(FORGET_EVERY);
    loop {
        tokio::select! {
            () = stop.cancelled() => return,
            _ = every.tick() => {}
        }
        let before = SystemTime::now()
            .checked_sub(KEEP)
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .and_then(|d| i64::try_from(d.as_nanos()).ok())
            .unwrap_or_default();
        match t.forget_before(before).await {
            Ok(n) => tracing::info!(removed = n, "telemetry kept 90 days"),
            Err(err) => tracing::error!(%err, "telemetry: remove old records"),
        }
    }
}

/// The door, routed under `PATTERN`.
pub fn router(o: Arc<Orchestrator>, t: Telemetry, dd: Datadog) -> Router {
    let door = Arc::new(Door { o, t, dd });
    let route = |decode: Decode, signal: Signal| {
        post(move |State(door): State<Arc<Door>>, req: Request| export(door, decode, signal, req))
    };
    Router::new()
        .route(&format!("{PATTERN}/v1/logs"), route(otlp::logs, Signal::Logs))
        .route(&format!("{PATTERN}/v1/metrics"), route(otlp::metrics, Signal::Metrics))
        .route(&format!("{PATTERN}/v1/traces"), route(otlp::traces, Signal::Traces))
        .with_state(door)
}

async fn export(door: Arc<Door>, decode: Decode, signal: Signal, req: Request) -> Response {
    let Some(caller) = bearer(&req).and_then(|token| door.o.caller_for_token(token)) else {
        return refusal();
    };
    let body: Bytes = match axum::body::to_bytes(req.into_body(), BODY_LIMIT).await {
        Ok(b) => b,
        Err(err) => return (StatusCode::PAYLOAD_TOO_LARGE, format!("{err}\n")).into_response(),
    };
    let records = match decode(&body) {
        Ok(r) => r,
        Err(err) => return (StatusCode::BAD_REQUEST, format!("{err}\n")).into_response(),
    };
    let (card, home) = (caller.card.id, caller.card.home);
    if let Err(err) = door.t.write(card.clone(), home.clone(), records).await {
        tracing::error!(%card, %err, "telemetry");
        return (StatusCode::SERVICE_UNAVAILABLE, "the telemetry could not be kept\n").into_response();
    }
    let who = Stamp {
        chain_id: home,
        card_id: card.clone(),
    };
    if let Err(err) = door.dd.forward(signal, &body, &who) {
        tracing::warn!(%card, %err, "telemetry: not sent to Datadog");
    }
    // An empty ExportServiceResponse: everything was taken.
    ([(CONTENT_TYPE, "application/json")], "{}").into_response()
}
