//! The server's errors as issues (ADR-0119): an ERROR it logs is grouped by its fingerprint (where it was logged and its
//! message with the numbers taken out), and the first of each issue is handed to the scheduler's side as
//! `fact:remit.error`, which a shipped subscription gives to a chain that investigates. A line that repeats an issue is
//! not handed again: the fact's dedupe keeps one per issue across restarts, this layer keeps a busy error from reaching
//! it at every line. It holds with Datadog absent. One ERROR is not an issue: rmcp's for an answer whose client had
//! left (`client_left`).

use std::collections::HashSet;
use std::sync::{Mutex, PoisonError};

use orch::shipping::remit_facts::RemitError;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

/// How many new issues wait to be recorded at most: the rest of a burst is raised by their next line.
const WAITING: usize = 256;
/// How many issues this process remembers having handed: past it, it forgets them all and the fact's dedupe holds.
const SEEN: usize = 10_000;

/// The layer, and where the issues it hands come out.
pub fn layer() -> (Issues, mpsc::Receiver<RemitError>) {
    let (tx, rx) = mpsc::channel(WAITING);
    (
        Issues {
            tx,
            seen: Mutex::new(HashSet::new()),
        },
        rx,
    )
}

pub struct Issues {
    tx: mpsc::Sender<RemitError>,
    seen: Mutex<HashSet<String>>,
}

/// The fingerprint of an issue: where it was logged and its message, digits taken out so the ids and counts a message
/// carries do not split one issue in many. Pure.
fn fingerprint(target: &str, message: &str) -> String {
    let mut template = String::with_capacity(message.len());
    let mut in_number = false;
    for c in message.chars() {
        if c.is_ascii_digit() {
            if !in_number {
                template.push('#');
            }
            in_number = true;
        } else {
            template.push(c);
            in_number = false;
        }
    }
    let digest = Sha256::digest(format!("{target}\n{template}").as_bytes());
    hex::encode(digest.get(..8).unwrap_or_default())
}

/// Whether an ERROR is rmcp's for an answer it could not send because the client of `/mcp` had closed its connection:
/// a session's process ended while it held the `subscriptions/listen` stream every Claude Code session opens, or gave
/// up on a call that then answered. rmcp 3.9 logs it at ERROR at every session end (`fail to response message`,
/// `failed to send pending response during drain`), and nothing in the server failed. It is told by where it was
/// logged and the error it names, which both lines share, so any other error of rmcp's stays an issue. Delete it with
/// its test when rmcp logs this below ERROR. Pure.
fn client_left(target: &str, error: &str) -> bool {
    target == "rmcp::service" && error == "channel closed"
}

impl<S> Layer<S> for Issues
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let meta = event.metadata();
        if *meta.level() != Level::ERROR {
            return;
        }
        let mut f = Fields::default();
        event.record(&mut f);
        if client_left(meta.target(), &f.error) {
            return;
        }
        let fp = fingerprint(meta.target(), &f.message);
        {
            let mut seen = self.seen.lock().unwrap_or_else(PoisonError::into_inner);
            if seen.len() >= SEEN {
                seen.clear();
            }
            if !seen.insert(fp.clone()) {
                return;
            }
        }
        // the chain and card the spans it was logged in name, as they set them where they opened
        let c = ctx
            .event_span(event)
            .and_then(|span| span.extensions().get::<telemetry::Correlation>().cloned())
            .unwrap_or_default();
        let get = |k| c.get(k).unwrap_or_default();
        let (chain, card) = (get(telemetry::attr::CHAIN_ID), get(telemetry::attr::CARD_ID));
        let sent = self.tx.try_send(RemitError {
            fingerprint: fp.clone(),
            target: meta.target().to_owned(),
            message: f.message,
            error: f.error,
            chain,
            card,
            ..RemitError::default()
        });
        if sent.is_err() {
            // the queue is full, and the work that logged it does not wait: a later line of the issue tries again
            self.seen.lock().unwrap_or_else(PoisonError::into_inner).remove(&fp);
        }
    }
}

/// The message of an event and the error it names.
#[derive(Default)]
struct Fields {
    message: String,
    error: String,
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.put(field, value.to_owned());
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.put(field, value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.put(field, format!("{value:?}"));
    }
}

impl Fields {
    fn put(&mut self, field: &Field, value: String) {
        match field.name() {
            "message" => self.message = value,
            "err" | "error" => self.error = value,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use axum::Router;
    use axum::body::Body;
    use axum::extract::Request;
    use axum::response::Response;
    use axum::routing::any;
    use rmcp::model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ServerCapabilities, ServerConfig,
        SubscriptionFilter,
    };
    use rmcp::service::{RequestContext, RoleServer, SubscriptionContext};
    use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
    use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
    use rmcp::{ErrorData, ServerHandler};
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use tokio::net::{TcpListener, TcpStream};
    use tower_service::Service as _;
    use tracing_subscriber::layer::SubscriberExt as _;

    use super::*;

    /// How long the tool below takes: the client leaves before it answers, and it answers inside rmcp's 2 s drain.
    const TOOL: Duration = Duration::from_millis(300);

    /// A card's server where a client that leaves meets it, as `chains::tools::CardServer` is: `listen` waits for its
    /// request's cancel and answers, and a tool never looks at the cancel.
    struct Card;

    impl ServerHandler for Card {
        fn get_info(&self) -> ServerConfig {
            ServerConfig::new(
                ServerCapabilities::builder()
                    .enable_tools()
                    .enable_tool_list_changed()
                    .build(),
            )
        }

        fn accepted_subscription_filter(&self, requested: &SubscriptionFilter) -> Option<SubscriptionFilter> {
            Some(requested.clone())
        }

        async fn listen(&self, ctx: SubscriptionContext) -> Result<(), ErrorData> {
            ctx.cancelled().await;
            Ok(())
        }

        async fn call_tool(
            &self,
            _: CallToolRequestParams,
            _: RequestContext<RoleServer>,
        ) -> Result<CallToolResponse, ErrorData> {
            tokio::time::sleep(TOOL).await;
            Ok(CallToolResult::success(vec![ContentBlock::text("done")]).into())
        }
    }

    /// The door as `agentmcp::router` configures it: stateless, a server per request, a JSON answer when it is the
    /// only message.
    async fn open(req: Request) -> Response {
        let config = StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_json_response(true);
        let mut mcp = StreamableHttpService::new(
            || Ok::<_, std::io::Error>(Card),
            Arc::new(NeverSessionManager::default()),
            config,
        );
        match mcp.call(req).await {
            Ok(res) => res.map(Body::new),
            Err(never) => match never {},
        }
    }

    /// The error of each ERROR `rmcp::service` logged, kept beside the layer under test.
    #[derive(Clone, Default)]
    struct Logged(Arc<Mutex<Vec<String>>>);

    impl<S: Subscriber> Layer<S> for Logged {
        fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
            let meta = event.metadata();
            if *meta.level() == Level::ERROR && meta.target() == "rmcp::service" {
                let mut f = Fields::default();
                event.record(&mut f);
                self.0.lock().unwrap().push(f.error);
            }
        }
    }

    impl Logged {
        /// Whether rmcp logged `n` answers it could not send within 2 s: both layers have seen each by then.
        async fn hang_ups(&self, n: usize) -> bool {
            let seen = async {
                while self.0.lock().unwrap().iter().filter(|e| *e == "channel closed").count() < n {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            };
            tokio::time::timeout(Duration::from_secs(2), seen).await.is_ok()
        }
    }

    /// Sends one request of a 2026-07-28 client, as Claude Code's are, on a connection of its own. `headers` are the
    /// lines the protocol asks of that method beside its name.
    async fn post(port: u16, method: &str, headers: &str, params: &str) -> TcpStream {
        let body = format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"{method}","params":{{{params},"_meta":{{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientInfo":{{"name":"test","version":"1"}},"io.modelcontextprotocol/clientCapabilities":{{}}}}}}}}"#
        );
        let request = format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\n\
             Accept: application/json, text/event-stream\r\nMCP-Protocol-Version: 2026-07-28\r\nMcp-Method: {method}\r\n\
             {headers}Content-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        client.write_all(request.as_bytes()).await.unwrap();
        client
    }

    #[tokio::test]
    async fn a_client_that_hung_up_raises_no_issue() {
        let (issues, mut issued) = layer();
        let logged = Logged::default();
        let _logging =
            tracing::subscriber::set_default(tracing_subscriber::registry().with(issues).with(logged.clone()));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, Router::new().route("/mcp", any(open))).await });

        // a session's process ends while it holds the stream every Claude Code session opens at connect
        let mut listening = post(
            port,
            "subscriptions/listen",
            "",
            r#""notifications":{"toolsListChanged":true}"#,
        )
        .await;
        let mut sent = Vec::new();
        while !String::from_utf8_lossy(&sent).contains("notifications/subscriptions/acknowledged") {
            let mut buf = [0; 1024];
            let n = tokio::time::timeout(Duration::from_secs(2), listening.read(&mut buf))
                .await
                .unwrap()
                .unwrap();
            assert_ne!(
                n,
                0,
                "the door closed before it acknowledged: {}",
                String::from_utf8_lossy(&sent)
            );
            sent.extend_from_slice(&buf[..n]);
        }
        drop(listening);
        assert!(
            logged.hang_ups(1).await,
            "rmcp logged no ERROR `channel closed` for the stream"
        );

        // a client gives up on a tool call that then answers
        let calling = post(
            port,
            "tools/call",
            "Mcp-Name: slow\r\n",
            r#""name":"slow","arguments":{}"#,
        )
        .await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        drop(calling);
        assert!(
            logged.hang_ups(2).await,
            "rmcp logged no ERROR `channel closed` for the tool call"
        );

        assert_eq!(issued.try_recv().ok(), None, "a client that left is not an issue");

        // any other error of rmcp's is one, under the very message a hang-up was logged with
        tracing::error!(target: "rmcp::service", error = "channel closed", "fail to response message");
        tracing::error!(target: "rmcp::service", error = "boom", "fail to response message");
        let issue = issued.try_recv().unwrap();
        assert_eq!((issue.target.as_str(), issue.error.as_str()), ("rmcp::service", "boom"));
        assert_eq!(issued.try_recv().ok(), None);

        // and so is a channel of Remit's own that closed: only where rmcp logs it is it a client that left
        tracing::error!(target: "chains::pipeline", error = "channel closed", "the reading was not delivered");
        let issue = issued.try_recv().unwrap();
        assert_eq!(
            (issue.target.as_str(), issue.error.as_str()),
            ("chains::pipeline", "channel closed")
        );
        assert_eq!(issued.try_recv().ok(), None);
    }
}
