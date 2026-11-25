use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, ready};

use bytes::Bytes;
use http::header::{self, HeaderMap, HeaderValue};
use http::{Request, Response, StatusCode, Uri};
use http_body::{Body as HttpBody, Frame};
use http_body_util::BodyExt;
use hyper::body::Incoming;
use hyper_util::rt::TokioIo;

use crate::egress::{Body, Inner, dial, text};
use crate::policy::new_target;
use crate::tunnel::Budget;

/// The headers that describe one connection, not the message: a proxy does not pass them on (RFC 9110, 7.6.1).
const HOP_BY_HOP: &[&str] = &[
    "connection",
    "proxy-connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

/// Removes the connection headers, and the ones the Connection header names, from h.
pub(crate) fn strip_hop_by_hop(h: &mut HeaderMap) {
    let named: Vec<String> = h
        .get_all(header::CONNECTION)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(',').map(|n| n.trim().to_string()))
        .collect();
    for name in named.iter().map(String::as_str).chain(HOP_BY_HOP.iter().copied()) {
        if let Ok(name) = header::HeaderName::from_bytes(name.as_bytes()) {
            h.remove(name);
        }
    }
}

impl Inner {
    /// Serves a plain HTTP request given as an absolute URI. The request goes to the address route approved and to no
    /// other, and no redirect is followed: a 3xx goes back to the program.
    pub(crate) async fn forward(self: Arc<Self>, req: Request<Incoming>) -> Response<Body> {
        let uri = req.uri().clone();
        let authority = uri.authority().map(|a| a.to_string()).unwrap_or_default();
        if uri.scheme().is_none() || authority.is_empty() {
            return text(
                StatusCode::BAD_REQUEST,
                "egress gate: this is a forward proxy; send an absolute URI, or CONNECT for https\n",
            );
        }
        if uri.scheme_str() != Some("http") {
            return self.refuse(
                &authority,
                StatusCode::FORBIDDEN,
                "only http:// is forwarded by absolute URI; https goes through CONNECT",
            );
        }
        if req.headers().contains_key(header::UPGRADE) {
            return self.refuse(
                &authority,
                StatusCode::FORBIDDEN,
                "a protocol upgrade is not relayed; use CONNECT",
            );
        }
        if !self.policy.allows_method(req.method().as_str()) {
            return self.refuse(
                &authority,
                StatusCode::FORBIDDEN,
                &format!("method {} is not allowed", req.method()),
            );
        }
        let host = uri.host().unwrap_or_default().trim_matches(['[', ']']);
        let port = uri.port().map_or_else(|| "80".to_string(), |p| p.as_str().to_string());
        let t = match new_target(host, &port) {
            Ok(t) => t,
            Err(e) => return self.refuse(&authority, StatusCode::FORBIDDEN, &format!("request target: {e}")),
        };
        let addrs = match self.route(&t).await {
            Ok(addrs) => addrs,
            Err((status, reason)) => return self.refuse(&t.to_string(), status, &reason),
        };

        let budget = Arc::new(Budget::new(self.cfg.max_bytes));
        let (mut parts, body) = req.into_parts();
        // The upstream is asked in origin form; the authority the Gate approved is the one it is asked for, whatever Host
        // header the program sent.
        parts.uri = uri
            .path_and_query()
            .map_or_else(|| Uri::from_static("/"), |pq| Uri::from(pq.clone()));
        strip_hop_by_hop(&mut parts.headers);
        if let Ok(v) = HeaderValue::from_str(&authority) {
            parts.headers.insert(header::HOST, v);
        }
        let out = Request::from_parts(parts, Capped::new(body, Arc::clone(&budget), None));
        let failed = || text(StatusCode::BAD_GATEWAY, &format!("egress gate: {t} did not answer\n"));
        let Ok(stream) = dial(&addrs, t.port).await else {
            return failed();
        };
        let resp = match send(self.tracker.clone(), TokioIo::new(stream), out).await {
            Ok(resp) => resp,
            Err(_) if budget.over() => {
                return self.refuse(&t.to_string(), StatusCode::FORBIDDEN, "request body over the byte cap");
            }
            Err(_) => return failed(),
        };
        if resp.body().size_hint().exact().is_some_and(|n| n > self.cfg.max_bytes) {
            return self.refuse(&t.to_string(), StatusCode::FORBIDDEN, "response over the byte cap");
        }
        let (mut parts, body) = resp.into_parts();
        strip_hop_by_hop(&mut parts.headers);
        let on_over = Some((Arc::clone(&self), t.to_string()));
        Response::from_parts(parts, Capped::new(body, budget, on_over).boxed())
    }
}

/// Sends one request over a new HTTP/1.1 connection and returns the answer; the connection is driven on the Gate's
/// tracker, so `close` waits for it.
pub(crate) async fn send<S, B>(
    tracker: tokio_util::task::TaskTracker,
    io: S,
    req: Request<B>,
) -> Result<Response<Incoming>, hyper::Error>
where
    S: hyper::rt::Read + hyper::rt::Write + Unpin + Send + 'static,
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;
    tracker.spawn(async move {
        #[expect(
            clippy::let_underscore_must_use,
            reason = "a connection that fails also fails the request on sender, which returns that error to the caller"
        )]
        let _ = conn.await;
    });
    sender.send_request(req).await
}

/// A body that carries bytes while the budget lasts and ends in an error past it. on_over is the Gate and target to
/// record the cut against, for an answer already on its way to the program: the error makes hyper drop the connection,
/// so the program does not take the end for complete.
pub(crate) struct Capped {
    inner: Incoming,
    budget: Arc<Budget>,
    on_over: Option<(Arc<Inner>, String)>,
}

impl Capped {
    pub(crate) fn new(inner: Incoming, budget: Arc<Budget>, on_over: Option<(Arc<Inner>, String)>) -> Capped {
        Capped { inner, budget, on_over }
    }
}

impl HttpBody for Capped {
    type Data = Bytes;
    type Error = io::Error;

    fn poll_frame(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Result<Frame<Bytes>, io::Error>>> {
        let this = self.get_mut();
        match ready!(Pin::new(&mut this.inner).poll_frame(cx)) {
            Some(Ok(frame)) => {
                if let Some(data) = frame.data_ref()
                    && !this.budget.spend(u64::try_from(data.len()).unwrap_or(u64::MAX))
                {
                    if let Some((gate, target)) = this.on_over.take() {
                        gate.denied.add(&target, "response ended at the byte cap");
                    }
                    return Poll::Ready(Some(Err(io::Error::other("byte cap reached"))));
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Some(Err(e)) => Poll::Ready(Some(Err(io::Error::other(e)))),
            None => Poll::Ready(None),
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}
