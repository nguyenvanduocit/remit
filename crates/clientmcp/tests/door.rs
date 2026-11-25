//! The door over a real HTTP connection: every refusal reads the same, and the key that opens it gets its tools over MCP.
#![expect(
    clippy::unwrap_used,
    reason = "test helpers outside #[test] fns, where unwrap is the assertion"
)]

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use clientmcp::{Keys, PATTERN};
use model::ClientKey;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ListToolsResult, PaginatedRequestParams,
    ServerCapabilities, ServerConfig, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData, ServerHandler};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const GOOD_KEY: &str = "ck_the-one-key-that-opens-it";

/// The application layer as the door sees it: one key opens it, and a key's tools are one tool.
#[derive(Default)]
struct FakeKeys {
    seen: Mutex<Vec<String>>,
}

#[derive(Debug)]
struct Never;

impl std::fmt::Display for Never {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("never")
    }
}

impl std::error::Error for Never {}

impl Keys for FakeKeys {
    type Server = WhoAmI;
    type Error = Never;

    async fn authorize_client_key(&self, presented: &str, addr: &str) -> Result<Option<ClientKey>, Never> {
        self.seen.lock().unwrap().push(addr.to_string());
        Ok((presented == GOOD_KEY).then(|| ClientKey {
            id: "ck_1".into(),
            name: "doctor".into(),
            ..ClientKey::default()
        }))
    }

    fn client_server(&self, key: ClientKey) -> WhoAmI {
        WhoAmI(key)
    }
}

struct WhoAmI(ClientKey);

impl ServerHandler for WhoAmI {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let schema = serde_json::Map::from_iter([("type".to_string(), "object".into())]);
        Ok(ListToolsResult::with_all_items(vec![Tool::new(
            "whoami", "the key", schema,
        )]))
    }

    async fn call_tool(
        &self,
        _: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        Ok(CallToolResult::success(vec![ContentBlock::text(self.0.name.clone())]).into())
    }
}

async fn serve(keys: Arc<FakeKeys>) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = clientmcp::router(keys).into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

/// Posts an MCP ping with the given extra header lines and returns the raw response, status line to body.
async fn post(addr: SocketAddr, host: &str, headers: &str) -> String {
    let body = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
    let req = format!(
        "POST {PATTERN} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\n\
         Accept: application/json, text/event-stream\r\nConnection: close\r\n{headers}Content-Length: {}\r\n\r\n{body}",
        body.len()
    );
    let mut conn = TcpStream::connect(addr).await.unwrap();
    conn.write_all(req.as_bytes()).await.unwrap();
    let mut res = String::new();
    conn.read_to_string(&mut res).await.unwrap();
    res
}

/// The status line and body of a raw response, leaving out the `date` header that differs between answers.
fn status_and_body(res: &str) -> (String, String) {
    let (head, body) = res.split_once("\r\n\r\n").unwrap();
    (head.lines().next().unwrap().to_string(), body.to_string())
}

#[tokio::test(flavor = "multi_thread")]
async fn every_refusal_is_the_same_and_echoes_nothing() {
    let addr = serve(Arc::default()).await;
    let here = addr.to_string();
    let good = format!("Authorization: Bearer {GOOD_KEY}\r\n");
    let refused = [
        (
            "a wrong key",
            here.clone(),
            "Authorization: Bearer ck_wrong\r\n".to_string(),
        ),
        ("no key", here.clone(), String::new()),
        (
            "a forwarded request",
            here.clone(),
            format!("{good}X-Forwarded-For: 203.0.113.9\r\n"),
        ),
        (
            "a Forwarded header",
            here.clone(),
            format!("{good}Forwarded: for=203.0.113.9\r\n"),
        ),
        (
            "an X-Real-IP header",
            here.clone(),
            format!("{good}X-Real-IP: 203.0.113.9\r\n"),
        ),
        (
            "a host that is not this machine",
            "remit.example.com".to_string(),
            good.clone(),
        ),
    ];
    for (name, host, headers) in refused {
        let res = post(addr, &host, &headers).await;
        let (status, body) = status_and_body(&res);
        assert_eq!(status, "HTTP/1.1 401 Unauthorized", "{name}: {res}");
        assert_eq!(body, "unauthorized\n", "{name}: {res}");
        assert!(res.to_lowercase().contains("www-authenticate: bearer"), "{name}: {res}");
        assert!(!res.contains("ck_"), "{name} echoes the key: {res}");
    }
    // The same request with the right key is let in, so the refusals above are the door's, not the transport's.
    let (status, _) = status_and_body(&post(addr, &here, &good).await);
    assert_eq!(status, "HTTP/1.1 200 OK");
}
