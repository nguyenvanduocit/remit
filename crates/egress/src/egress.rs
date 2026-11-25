use std::convert::Infallible;
use std::future::Future;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use http::{Method, Request, Response, StatusCode, header};
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::net::{TcpStream, UnixListener};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use crate::denied::{Denial, Denials};
use crate::inject::{Injection, Tunnelled};
use crate::policy::{Policy, PolicyError, Target, routable};

/// The body of every answer the Gate gives: its own text, or an upstream's bytes as they come.
pub(crate) type Body = BoxBody<Bytes, io::Error>;

/// Turns a name into addresses. `SystemResolver` is the system's; a test supplies another that answers as an attacker's
/// DNS would.
pub trait Resolver: Send + Sync {
    fn lookup_ip<'a>(&'a self, host: &'a str) -> Pin<Box<dyn Future<Output = io::Result<Vec<IpAddr>>> + Send + 'a>>;
}

/// The system's resolver (getaddrinfo, through tokio).
pub struct SystemResolver;

impl Resolver for SystemResolver {
    fn lookup_ip<'a>(&'a self, host: &'a str) -> Pin<Box<dyn Future<Output = io::Result<Vec<IpAddr>>> + Send + 'a>> {
        Box::pin(async move { Ok(tokio::net::lookup_host((host, 0)).await?.map(|a| a.ip()).collect()) })
    }
}

/// Everything the caller fixes about a Gate. None of it can change while the Gate runs.
#[derive(Clone)]
pub struct Config {
    /// The host:port pairs a program may connect to: names or address literals, one port each.
    pub allow: Vec<String>,
    /// When not empty, the methods a plain HTTP request may use (upper case). It does not govern CONNECT, whose bytes the
    /// Gate does not read; empty allows every method.
    pub methods: Vec<String>,
    /// Ends a tunnel or a plain HTTP request, whatever it is doing, this long after it was asked for.
    pub deadline: Duration,
    /// How many bytes a tunnel or a plain HTTP exchange may carry, both directions together.
    pub max_bytes: u64,
    /// How many tunnels and requests are served at once; the next is refused with 503.
    pub max_tunnels: usize,
    /// How allowed names are resolved; None is `SystemResolver`.
    pub resolver: Option<Arc<dyn Resolver>>,
}

/// Why a Gate or an Injector did not start.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Policy(#[from] PolicyError),
    #[error("egress: {0}")]
    Config(&'static str),
    #[error("egress: listen: {0}")]
    Listen(#[source] io::Error),
    #[error("egress: socket mode: {0}")]
    SocketMode(#[source] io::Error),
    #[error("egress: injector: {0}")]
    Injector(String),
}

impl Config {
    fn validate(&self) -> Result<Policy, Error> {
        if self.deadline.is_zero() {
            return Err(Error::Config("deadline must be positive: every tunnel has one"));
        }
        if self.max_bytes == 0 {
            return Err(Error::Config("max bytes must be positive"));
        }
        if self.max_tunnels == 0 {
            return Err(Error::Config("max tunnels must be positive"));
        }
        Ok(Policy::new(&self.allow, &self.methods)?)
    }
}

/// A forward proxy on a unix socket. Make one with `Gate::new` and end it with `close`; dropping it stops it too, without
/// waiting.
pub struct Gate {
    socket: PathBuf,
    pub(crate) inner: Arc<Inner>,
}

/// What every connection of a Gate shares.
pub(crate) struct Inner {
    pub(crate) cfg: Config,
    pub(crate) policy: Policy,
    resolver: Arc<dyn Resolver>,
    pub(crate) denied: Denials,
    state: Mutex<State>,
    pub(crate) token: CancellationToken,
    pub(crate) tracker: TaskTracker,
    /// Set on a Gate made by `Injector::new`: it then ends TLS and injects instead of tunnelling.
    pub(crate) inject: Option<Injection>,
}

#[derive(Default)]
struct State {
    closed: bool,
    active: usize,
}

impl Gate {
    /// Starts a Gate on a new unix socket at path (the file must not exist; put it in a directory only the server's user
    /// can enter). It runs on the current tokio runtime until `close` is called or it is dropped.
    pub async fn new(path: &Path, cfg: Config) -> Result<Gate, Error> {
        start(path, cfg, None).await
    }

    /// The path of the unix socket: the value for the jail's socket.
    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// What the Gate refused so far, each target and reason once with how often. At most 64 distinct pairs are kept; a
    /// last entry counts the others.
    pub fn denied(&self) -> Vec<Denial> {
        self.inner.denied.all()
    }

    /// Stops the Gate: the socket is closed, every tunnel is ended and the call returns when none is left running. It may
    /// be called again.
    pub async fn close(&self) {
        self.inner.lock().closed = true;
        self.inner.token.cancel();
        self.inner.tracker.close();
        self.inner.tracker.wait().await;
        // The socket file is unlinked on close; tokio's listener does not do it.
        #[expect(
            clippy::let_underscore_must_use,
            reason = "close reports nothing: a second close finds the file gone, and a file left behind is reported by the next bind of the path"
        )]
        let _ = std::fs::remove_file(&self.socket);
    }
}

impl Drop for Gate {
    fn drop(&mut self) {
        self.inner.token.cancel();
    }
}

/// `Gate::new`, for a Gate that injects when inj is set.
pub(crate) async fn start(path: &Path, cfg: Config, inject: Option<Injection>) -> Result<Gate, Error> {
    let policy = cfg.validate()?;
    let listener = UnixListener::bind(path).map_err(Error::Listen)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(Error::SocketMode)?;
    let resolver = cfg.resolver.clone().unwrap_or_else(|| Arc::new(SystemResolver));
    let inner = Arc::new(Inner {
        cfg,
        policy,
        resolver,
        denied: Denials::default(),
        state: Mutex::new(State::default()),
        token: CancellationToken::new(),
        tracker: TaskTracker::new(),
        inject,
    });
    let serving = Arc::clone(&inner);
    inner.tracker.spawn(async move {
        loop {
            let stream = tokio::select! {
                () = serving.token.cancelled() => return,
                accepted = listener.accept() => match accepted {
                    Ok((stream, _)) => stream,
                    Err(_) => continue,
                },
            };
            let conn = Arc::clone(&serving);
            serving
                .tracker
                .spawn(async move { conn.serve_connection(stream, None).await });
        }
    });
    Ok(Gate {
        socket: path.to_path_buf(),
        inner,
    })
}

impl Inner {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        // A poisoned lock still holds whole counts: every change is one step.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Serves HTTP/1.1 on one connection of a program until it ends or the Gate closes. inside is the CONNECT whose TLS
    /// the Injector ended: its requests go to the exchange with its destination, under its deadline, not through the door.
    pub(crate) async fn serve_connection<S>(self: Arc<Self>, io: S, inside: Option<Tunnelled>)
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let me = Arc::clone(&self);
        let service = service_fn(move |req| {
            let inner = Arc::clone(&me);
            let inside = inside.clone();
            async move {
                let Some(inside) = inside else {
                    return Ok::<_, Infallible>(inner.serve(req).await);
                };
                let gate = Arc::clone(&inner);
                let exchanged = gate.exchange(&inside.destination, req);
                Ok(match tokio::time::timeout_at(inside.deadline, exchanged).await {
                    Ok(resp) => resp,
                    Err(_) => text(StatusCode::BAD_GATEWAY, "egress gate: the deadline passed\n"),
                })
            }
        });
        let conn = hyper::server::conn::http1::Builder::new()
            .timer(TokioTimer::new())
            .header_read_timeout(Duration::from_secs(10))
            .max_buf_size(16 << 10)
            .serve_connection(TokioIo::new(io), service)
            .with_upgrades();
        tokio::select! {
            () = self.token.cancelled() => {}
            _ = conn => {}
        }
    }

    /// Takes the request through the one door: admit it, then tunnel (CONNECT) or forward (an absolute URI).
    async fn serve(self: Arc<Self>, req: Request<Incoming>) -> Response<Body> {
        let host = req.uri().authority().map(|a| a.to_string()).unwrap_or_default();
        let turn = match self.enter() {
            Ok(turn) => turn,
            Err(reason) => return self.refuse(&host, StatusCode::SERVICE_UNAVAILABLE, reason),
        };
        let Some(deadline) = Instant::now().checked_add(self.cfg.deadline) else {
            return text(
                StatusCode::INTERNAL_SERVER_ERROR,
                "egress gate: the deadline is past what the clock can hold\n",
            );
        };
        if self.inject.is_some() {
            return self.serve_injected(req, deadline, turn).await;
        }
        if req.method() == Method::CONNECT {
            return self.connect(req, deadline, turn).await;
        }
        match tokio::time::timeout_at(deadline, self.forward(req)).await {
            Ok(resp) => resp,
            Err(_) => text(
                StatusCode::BAD_GATEWAY,
                &format!("egress gate: {host} did not answer\n"),
            ),
        }
    }

    /// Admits one tunnel or request: the reason when the Gate is closing or has `max_tunnels` open. The turn ends when
    /// the returned value is dropped.
    fn enter(self: &Arc<Self>) -> Result<Turn, &'static str> {
        let mut s = self.lock();
        if s.closed {
            return Err("the gate is closing");
        }
        if s.active >= self.cfg.max_tunnels {
            return Err("too many tunnels open at once");
        }
        // Below max_tunnels, so it cannot saturate; Turn's drop takes it back down.
        s.active = s.active.saturating_add(1);
        Ok(Turn(Arc::clone(self)))
    }

    /// Answers a request the Gate will not serve. A refusal by rule (403) or by capacity (503) is recorded for the answer
    /// to the owner; a name that does not resolve (502) is the network's failure, not a decision of the Gate.
    pub(crate) fn refuse(&self, target: &str, status: StatusCode, reason: &str) -> Response<Body> {
        if status != StatusCode::BAD_GATEWAY {
            self.denied.add(target, reason);
        }
        text(status, &format!("egress gate: {reason}\n"))
    }

    /// Where a program is sent for t: the addresses to dial, or the refusal. A name is resolved here, once, and the
    /// addresses checked: the connection goes to one of these and to nothing resolved again.
    pub(crate) async fn route(&self, t: &Target) -> Result<Vec<IpAddr>, (StatusCode, String)> {
        if !self.policy.allows(t) {
            return Err((StatusCode::FORBIDDEN, format!("{t} is not on the allowlist")));
        }
        if let Some(ip) = t.ip {
            return Ok(vec![ip]);
        }
        let found = match self.resolver.lookup_ip(&t.host).await {
            Ok(found) if !found.is_empty() => found,
            _ => return Err((StatusCode::BAD_GATEWAY, format!("cannot resolve {}", t.host))),
        };
        let mut addrs = Vec::with_capacity(found.len());
        for a in found {
            let a = a.to_canonical();
            if !routable(a) {
                return Err((
                    StatusCode::FORBIDDEN,
                    format!("{} resolves to {a}, an address of this machine or its network", t.host),
                ));
            }
            addrs.push(a);
        }
        Ok(addrs)
    }
}

/// One admitted tunnel or request, counted against `max_tunnels` until it is dropped.
pub(crate) struct Turn(Arc<Inner>);

impl Drop for Turn {
    fn drop(&mut self) {
        let mut s = self.0.lock();
        s.active = s.active.saturating_sub(1);
    }
}

/// Connects to the first address that answers. With no address it fails.
pub(crate) async fn dial(addrs: &[IpAddr], port: u16) -> io::Result<TcpStream> {
    let mut failed = io::Error::new(io::ErrorKind::NotFound, "no address to connect to");
    for a in addrs {
        match TcpStream::connect(SocketAddr::new(*a, port)).await {
            Ok(c) => return Ok(c),
            Err(e) => failed = e,
        }
    }
    Err(failed)
}

/// An answer of the Gate's own, in plain text.
pub(crate) fn text(status: StatusCode, body: &str) -> Response<Body> {
    let mut resp = Response::new(full(Bytes::from(body.to_string())));
    *resp.status_mut() = status;
    let h = resp.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        header::HeaderValue::from_static("nosniff"),
    );
    resp
}

pub(crate) fn full(b: Bytes) -> Body {
    Full::new(b).map_err(|never| match never {}).boxed()
}

#[cfg(test)]
pub(crate) mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, UnixStream};

    use super::*;

    pub(crate) fn base_config(allow: &[&str]) -> Config {
        Config {
            allow: allow.iter().map(|s| s.to_string()).collect(),
            methods: Vec::new(),
            deadline: Duration::from_secs(10),
            max_bytes: 1 << 20,
            max_tunnels: 16,
            resolver: None,
        }
    }

    /// A short socket path: a unix socket path is limited to about 100 bytes.
    pub(crate) fn socket_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("eg-{}-{name}.sock", std::process::id()))
    }

    /// A plain HTTP server on loopback that answers every request with its path, and records each request as it arrived,
    /// headers included.
    async fn origin() -> (String, Arc<Mutex<Vec<String>>>) {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap().to_string();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&seen);
        tokio::spawn(async move {
            while let Ok((mut c, _)) = l.accept().await {
                let mut buf = vec![0; 4096];
                let n = c.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                let path = req.split(' ').nth(1).unwrap_or("").to_string();
                record.lock().unwrap().push(req);
                let body = format!("hello from {path}");
                #[expect(
                    clippy::let_underscore_must_use,
                    reason = "a client that hung up before the answer is the test's to notice, not this stub's"
                )]
                let _ = c
                    .write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes())
                    .await;
            }
        });
        (addr, seen)
    }

    /// What a program's HTTP client sends a plain-HTTP proxy, over the gate's socket; the whole answer comes back.
    async fn ask_through(g: &Gate, method: &str, url: &str, host: &str) -> String {
        let mut c = UnixStream::connect(g.socket()).await.unwrap();
        c.write_all(format!("{method} {url} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut out = Vec::new();
        c.read_to_end(&mut out).await.unwrap();
        String::from_utf8_lossy(&out).into_owned()
    }

    #[tokio::test]
    async fn a_plain_http_request_to_an_allowed_host_is_forwarded_and_any_other_is_refused() {
        let (site, _) = origin().await;
        let (elsewhere, elsewhere_seen) = origin().await;
        let path = socket_path("fwd");
        let g = Gate::new(&path, base_config(&[&site])).await.unwrap();

        let got = ask_through(&g, "GET", &format!("http://{site}/page"), &site).await;
        assert!(
            got.starts_with("HTTP/1.1 200") && got.ends_with("hello from /page"),
            "GET through the gate: {got:?}"
        );
        for url in [
            format!("http://{elsewhere}/"),
            "http://127.0.0.1:1/".to_string(),
            "http://example.com/".to_string(),
        ] {
            let got = ask_through(&g, "GET", &url, "x").await;
            assert!(
                got.starts_with("HTTP/1.1 403") && got.contains("egress gate"),
                "{url}: {got:?}"
            );
        }
        assert!(
            elsewhere_seen.lock().unwrap().is_empty(),
            "a server not allowed was asked"
        );
        assert_eq!(g.denied().len(), 3, "Denied = {:?}, want the 3 refused", g.denied());
        g.close().await;
    }

    /// Stands for two connections' credentials: each must reach its own upstream and nothing else.
    const TOKEN_A: &str = "tok-a-5d1e9c";
    const TOKEN_B: &str = "tok-b-77f02a";

    fn upstream(base: &str, header: &str, value: &str, writes: bool) -> crate::Upstream {
        crate::Upstream {
            base: base.to_string(),
            headers: vec![(header.to_string(), value.to_string())],
            sign: None,
            read_posts: Vec::new(),
            writes,
            scrub: Arc::new(|s: &str| s.replace(TOKEN_A, "[a]").replace(TOKEN_B, "[b]")),
            max_request_body: 1 << 16,
            max_answer: 1 << 16,
        }
    }

    fn inject_config(upstreams: Vec<crate::Upstream>) -> crate::InjectConfig {
        crate::InjectConfig {
            upstreams,
            deadline: Duration::from_secs(10),
            max_bytes: 1 << 20,
            max_requests: 50,
            max_tunnels: 8,
            // Both upstreams are plain http: no certificate is checked, and the system's roots are not read.
            roots: Some(Arc::new(rustls::RootCertStore::empty())),
            resolver: None,
            now: Arc::new(std::time::SystemTime::now),
        }
    }

    #[tokio::test]
    async fn one_run_reaches_each_connection_with_its_own_credential_and_writes_only_where_granted() {
        let (read_site, read_seen) = origin().await;
        let (write_site, write_seen) = origin().await;
        let reads = upstream(
            &format!("http://{read_site}"),
            "Authorization",
            &format!("Bearer {TOKEN_A}"),
            false,
        );
        let writes = upstream(&format!("http://{write_site}"), "X-Api-Key", TOKEN_B, true);
        let twice = crate::Injector::new(
            &socket_path("inj-twice"),
            inject_config(vec![reads.clone(), reads.clone()]),
        )
        .await;
        assert!(twice.is_err(), "two upstreams on one host and port started");
        let inj = crate::Injector::new(&socket_path("inj-two"), inject_config(vec![reads, writes]))
            .await
            .unwrap();

        let got = ask_through(&inj, "GET", &format!("http://{read_site}/dash"), &read_site).await;
        assert!(
            got.starts_with("HTTP/1.1 200") && got.ends_with("hello from /dash"),
            "{got:?}"
        );
        let got = ask_through(&inj, "PUT", &format!("http://{write_site}/items/1"), &write_site).await;
        assert!(
            got.starts_with("HTTP/1.1 200") && got.ends_with("hello from /items/1"),
            "{got:?}"
        );
        let got = ask_through(&inj, "PUT", &format!("http://{read_site}/items/1"), &read_site).await;
        assert!(
            got.starts_with("HTTP/1.1 403"),
            "a write to the read connection: {got:?}"
        );
        let got = ask_through(&inj, "GET", "http://127.0.0.1:1/", "127.0.0.1:1").await;
        assert!(got.starts_with("HTTP/1.1 403"), "a host of no connection: {got:?}");

        let read_seen = read_seen.lock().unwrap().clone();
        assert_eq!(read_seen.len(), 1, "the read upstream saw {read_seen:?}");
        let req = read_seen[0].to_ascii_lowercase();
        assert!(req.starts_with("get /dash http/1.1"), "{req}");
        assert!(req.contains(&format!("authorization: bearer {TOKEN_A}")), "{req}");
        assert!(
            !req.contains(TOKEN_B),
            "the read upstream saw the other credential: {req}"
        );
        let write_seen = write_seen.lock().unwrap().clone();
        assert_eq!(write_seen.len(), 1, "the write upstream saw {write_seen:?}");
        let req = write_seen[0].to_ascii_lowercase();
        assert!(req.starts_with("put /items/1 http/1.1"), "{req}");
        assert!(req.contains(&format!("x-api-key: {TOKEN_B}")), "{req}");
        assert!(
            !req.contains(TOKEN_A),
            "the write upstream saw the other credential: {req}"
        );
        inj.close().await;
    }
}
