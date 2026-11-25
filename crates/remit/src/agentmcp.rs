//! The door every agent's session reaches Remit's tools through (`/mcp`, which orch's port leaves to the crate that
//! holds the door). A request comes in only with the bearer token of a live run, or of a seat of one
//! (`Orchestrator::caller_for_token`), and is served the tools of that card (`server_for`) or seat (`seat_server_for`).
//! Stateless, as `/mcp/client` is: every request is checked and served by its own token's server, so another card's
//! token can never use a session this one opened.

use std::sync::{Arc, Mutex, PoisonError};

use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE, WWW_AUTHENTICATE};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use orch::{CardServer, Orchestrator};
use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use tower_service::Service as _;

/// Where the door is mounted (`orch::Config::mcp_url` names it).
pub const PATTERN: &str = "/mcp";

struct Door {
    o: Arc<Orchestrator>,
    sessions: Arc<NeverSessionManager>,
    config: StreamableHttpServerConfig,
}

/// The door, routed at `PATTERN`.
pub fn router(o: Arc<Orchestrator>) -> Router {
    let door = Door {
        o,
        sessions: Arc::new(NeverSessionManager::default()),
        config: StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_json_response(true),
    };
    Router::new().route(PATTERN, any(open)).with_state(Arc::new(door))
}

async fn open(State(door): State<Arc<Door>>, req: Request) -> Response {
    let Some(token) = bearer(&req) else {
        return refusal();
    };
    let Some(caller) = door.o.caller_for_token(token) else {
        return refusal();
    };
    let server = if caller.seat.is_empty() {
        door.o.server_for(&caller.card.id).await
    } else {
        door.o.seat_server_for(&caller.card.id, &caller.seat).await
    };
    // None: what its tools depend on (the card, its chain) cannot be read; refused rather than served with tools missing.
    let Some(server) = server else {
        return (StatusCode::SERVICE_UNAVAILABLE, "the card's tools cannot be read\n").into_response();
    };
    // A stateless request makes its server once; the service asks its factory, which hands that one over.
    let one = Mutex::new(Some(server));
    let mut mcp = StreamableHttpService::new(move || take(&one), Arc::clone(&door.sessions), door.config.clone());
    match mcp.call(req).await {
        Ok(res) => res.map(Body::new),
        Err(never) => match never {},
    }
}

fn take(one: &Mutex<Option<CardServer>>) -> Result<CardServer, std::io::Error> {
    one.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
        .ok_or_else(|| std::io::Error::other("a request is served by one server"))
}

pub(crate) fn bearer(req: &Request) -> Option<&str> {
    let value = req.headers().get(AUTHORIZATION)?.to_str().ok()?;
    value.strip_prefix("Bearer ").filter(|t| !t.is_empty())
}

/// The one answer to a token that opens nothing.
pub(crate) fn refusal() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [
            (WWW_AUTHENTICATE, "Bearer"),
            (CONTENT_TYPE, "text/plain; charset=utf-8"),
        ],
        "unauthorized\n",
    )
        .into_response()
}
