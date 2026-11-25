//! The door an outside agent holding a client key reaches Remit through (`/mcp/client`): a channel like the browser's and
//! the door of the event signals, thin on purpose. A request is let in only from this machine and only with a key the owner
//! handed out; what the holder may then do is the MCP server the application layer gives that key
//! ([`Keys::client_server`]), whose limits and rails the orchestrator keeps, not this door.

use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE, HOST, WWW_AUTHENTICATE, X_CONTENT_TYPE_OPTIONS};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use model::ClientKey;
use rmcp::ServerHandler;
use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use tower_service::Service;

/// Where the door is mounted.
pub const PATTERN: &str = "/mcp/client";

/// What the door needs of Remit: whether a presented key opens it, and the MCP server of the key that does. An error from
/// `authorize_client_key` is the server's.
pub trait Keys: Send + Sync + 'static {
    /// The tools one key may use.
    type Server: ServerHandler + Send + 'static;
    type Error: std::error::Error + Send;

    /// The key `presented` names, when it opens the door; `addr` is where it was presented from, for the key's page.
    fn authorize_client_key(
        &self,
        presented: &str,
        addr: &str,
    ) -> impl Future<Output = Result<Option<ClientKey>, Self::Error>> + Send;

    fn client_server(&self, key: ClientKey) -> Self::Server;
}

/// The headers a proxy adds to a request it passes on. The endpoint answers this machine only, so a request that carries
/// one came from further away than its remote address says.
const FORWARDED_HEADERS: [&str; 3] = ["forwarded", "x-forwarded-for", "x-real-ip"];

/// The door, routed at [`PATTERN`]. It reads the remote address from axum's `ConnectInfo<SocketAddr>`, so the server is
/// served with `into_make_service_with_connect_info`; without it every request is refused.
pub fn router<K: Keys>(keys: Arc<K>) -> Router {
    let door = Door {
        keys,
        sessions: Arc::new(NeverSessionManager::default()),
        // Stateless: every request is checked here and served by its own key's server, so a session opened by one key is
        // never used by another. The door checks the host itself, so every refusal reads the same.
        config: StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_json_response(true)
            .disable_allowed_hosts(),
    };
    Router::new().route(PATTERN, any(open::<K>)).with_state(Arc::new(door))
}

struct Door<K> {
    keys: Arc<K>,
    sessions: Arc<NeverSessionManager>,
    config: StreamableHttpServerConfig,
}

async fn open<K: Keys>(State(door): State<Arc<Door<K>>>, req: Request) -> Response {
    let Some(remote) = req.extensions().get::<ConnectInfo<SocketAddr>>().map(|c| c.0.ip()) else {
        return refusal();
    };
    if !from_this_machine(remote, host(&req), req.headers()) {
        return refusal();
    }
    let Some(token) = bearer(req.headers()) else {
        return refusal();
    };
    let key = match door.keys.authorize_client_key(token, &remote.to_string()).await {
        Ok(Some(key)) => key,
        Ok(None) => return refusal(),
        Err(err) => {
            // The holder is refused as for any key; the owner reads why here.
            tracing::error!(%err, "client key not checked");
            return refusal();
        }
    };
    let keys = Arc::clone(&door.keys);
    let mut mcp = StreamableHttpService::new(
        move || Ok(keys.client_server(key.clone())),
        Arc::clone(&door.sessions),
        door.config.clone(),
    );
    match mcp.call(req).await {
        Ok(res) => res.map(Body::new),
        Err(never) => match never {},
    }
}

/// The one answer to every refusal, which repeats nothing the request presented.
fn refusal() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [
            (WWW_AUTHENTICATE, "Bearer"),
            (CONTENT_TYPE, "text/plain; charset=utf-8"),
            (X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        "unauthorized\n",
    )
        .into_response()
}

/// The token of an `Authorization: Bearer <token>` header, when there is one.
fn bearer(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(AUTHORIZATION)?.to_str().ok()?;
    value.strip_prefix("Bearer ").filter(|t| !t.is_empty())
}

/// The host the request names: the request target's authority, else its `Host` header.
fn host(req: &Request) -> &str {
    if let Some(authority) = req.uri().authority() {
        return authority.as_str();
    }
    req.headers()
        .get(HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default()
}

/// Says a request came straight from a process on this machine, for this machine: a loopback remote address, no proxy
/// header, and a host that names this machine.
fn from_this_machine(remote: IpAddr, host: &str, headers: &HeaderMap) -> bool {
    if !remote.to_canonical().is_loopback() {
        return false;
    }
    if FORWARDED_HEADERS
        .iter()
        .any(|h| headers.get(*h).is_some_and(|v| !v.is_empty()))
    {
        return false;
    }
    let host = without_port(host).trim_matches(['[', ']']);
    host == "localhost" || host.parse::<IpAddr>().is_ok_and(|ip| ip.to_canonical().is_loopback())
}

/// The host of `host:port` or `[host]:port`; anything else is returned whole.
fn without_port(authority: &str) -> &str {
    if let Some(rest) = authority.strip_prefix('[') {
        return rest.split_once("]:").map_or(authority, |(h, _)| h);
    }
    match authority.split_once(':') {
        Some((h, port)) if !port.contains(':') => h,
        _ => authority,
    }
}
