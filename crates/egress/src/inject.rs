//! The injecting mode (ADR-0018, form T1; ADR-0116, *Credentials*): the same unix-socket proxy as the Gate, but its
//! destinations are the connections of one run and it does not tunnel to them. A CONNECT, or a request in absolute form,
//! names a connection by its host and port; the injector ends the program's TLS itself with a certificate made for that
//! connection in this run, reads every request, applies that connection's rules (inject_rules: reads only, or writes as
//! its grant allows), adds the credential headers the caller fixed for it (replacing any the program sent) and asks the
//! real upstream over a fresh connection of its own. The program never holds a credential, and what it may say to each
//! upstream is what that upstream's rules admit. The request count and the byte budget are the run's, shared by every
//! connection.
//!
//! Redirects are never followed. A 3xx is passed back unchanged, unless it sends the program to another host: then it is
//! replaced by a 502 that says so. An answer is held in memory, up to a cap, so that the credential can be removed from
//! it whole (Scrub). Set-Cookie and WWW-Authenticate are never passed on.

use std::error::Error as StdError;
use std::future::Future;
use std::ops::Deref;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::{Duration, SystemTime};

use http::header::{HeaderName, HeaderValue};
use http::{Method, Request, Response, StatusCode, Uri};
use http_body_util::{BodyExt, Empty};
use hyper::body::Incoming;
use hyper_util::rt::TokioIo;
use rustls::RootCertStore;
use tokio::time::Instant;
use tokio_rustls::{TlsAcceptor, TlsConnector};

use crate::denied::showable;
use crate::egress::{Body, Config, Error, Gate, Inner, Resolver, Turn, start};
use crate::inject_cert::server_identity;
use crate::inject_exchange::Refusal;
use crate::inject_rules::{RequestRules, same_authority};
use crate::policy::{Target, new_target, parse_target, split_host_port};
use crate::tunnel::Budget;

/// Makes a credential from the request itself (an AWS signature covers its method, path, query, headers and body): it is
/// called on the request the upstream will receive (its URI absolute), after the headers are set, with the whole body and
/// the run's time. An error sends nothing.
pub type Sign = Arc<
    dyn Fn(&mut http::request::Parts, &[u8], SystemTime) -> Result<(), Box<dyn StdError + Send + Sync>> + Send + Sync,
>;

/// Removes the credential from text the upstream answered with, whatever form it echoes it in.
pub type Scrub = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// Everything the caller fixes about one run of an Injector. None of it can change while the Injector runs.
#[derive(Clone)]
pub struct InjectConfig {
    /// The destinations of this run, one per connection: at least one; two with the same host and port are refused.
    pub upstreams: Vec<Upstream>,
    /// Ends a tunnel or request this long after it was asked for.
    pub deadline: Duration,
    /// How many bytes of bodies, both directions together, the run carries across every upstream.
    pub max_bytes: u64,
    /// How many requests the run serves across every upstream, refused ones included; the next is answered 429.
    pub max_requests: u64,
    /// How many connections are served at once.
    pub max_tunnels: usize,
    /// The authorities an upstream's certificate is checked against; None is the system's.
    pub roots: Option<Arc<RootCertStore>>,
    /// How an upstream's name is resolved; None is the system's resolver.
    pub resolver: Option<Arc<dyn Resolver>>,
    /// Dates the run's certificates and is the time a signer is given.
    pub now: Arc<dyn Fn() -> SystemTime + Send + Sync>,
}

/// One connection a run reaches: its address, its credential and what a program may say to it.
#[derive(Clone)]
pub struct Upstream {
    /// The connection's address: scheme://host[:port][/prefix]. A request path is placed under its prefix. It holds no
    /// login.
    pub base: String,
    /// The credential: set on every request to the upstream, replacing what the program sent.
    pub headers: Vec<(String, String)>,
    /// Makes a credential from the request; the caller gives headers or sign, or both.
    pub sign: Option<Sign>,
    /// The paths where POST only reads (a search that takes its query in a body); on a connection that only reads, every
    /// other path takes GET alone.
    pub read_posts: Vec<String>,
    /// Admits writes: every method but CONNECT and TRACE passes the path rules (POST anywhere, PUT, PATCH, DELETE);
    /// false: reads only.
    pub writes: bool,
    pub scrub: Scrub,
    /// The largest body of one request; max_answer the largest answer to one.
    pub max_request_body: u64,
    pub max_answer: u64,
}

/// A Gate that injects. Make one with `Injector::new` and end it with `close`.
pub struct Injector {
    gate: Gate,
    ca_pem: Vec<u8>,
}

impl Deref for Injector {
    type Target = Gate;
    fn deref(&self) -> &Gate {
        &self.gate
    }
}

/// The state of a Gate in injecting mode: the run's destinations and what they share.
pub(crate) struct Injection {
    destinations: Vec<Arc<Destination>>,
    pub(crate) now: Arc<dyn Fn() -> SystemTime + Send + Sync>,
    pub(crate) max_requests: u64,
    pub(crate) requests: AtomicU64,
    pub(crate) budget: Budget,
}

/// One upstream of a run, made ready to serve.
pub(crate) struct Destination {
    pub(crate) https: bool,
    /// The address's authority as written: the Host header and the URL the upstream is asked at.
    pub(crate) base_host: String,
    server_name: rustls_pki_types::ServerName<'static>,
    pub(crate) target: Target,
    pub(crate) rules: RequestRules,
    pub(crate) headers: Vec<(HeaderName, HeaderValue)>,
    pub(crate) sign: Option<Sign>,
    pub(crate) scrub: Scrub,
    acceptor: TlsAcceptor,
    connector: TlsConnector,
    pub(crate) max_request_body: u64,
    pub(crate) max_answer: u64,
}

impl Injector {
    /// Starts an Injector on a new unix socket at path, under the rules of `Gate::new`. It makes a certificate authority
    /// for each upstream of the run; `certificate_pem` is what a program trusts.
    pub async fn new(path: &Path, cfg: InjectConfig) -> Result<Injector, Error> {
        let (inj, ca_pem) = new_injection(&cfg).map_err(Error::Injector)?;
        let gate_cfg = Config {
            allow: inj.destinations.iter().map(|d| d.target.to_string()).collect(),
            methods: Vec::new(),
            deadline: cfg.deadline,
            max_bytes: cfg.max_bytes,
            max_tunnels: cfg.max_tunnels,
            resolver: cfg.resolver.clone(),
        };
        let gate = start(path, gate_cfg, Some(inj)).await?;
        Ok(Injector { gate, ca_pem })
    }

    /// The authorities a program must trust to talk to the Injector, one per upstream, as one PEM bundle: certificates,
    /// never a key. Fresh for every Injector.
    pub fn certificate_pem(&self) -> Vec<u8> {
        self.ca_pem.clone()
    }
}

fn new_injection(c: &InjectConfig) -> Result<(Injection, Vec<u8>), String> {
    if c.upstreams.is_empty() {
        return Err("no upstream: an injector that reaches nothing is not started".to_string());
    }
    if c.max_requests == 0 {
        return Err("max requests must be positive".to_string());
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let roots = match &c.roots {
        Some(r) => Arc::clone(r),
        None => {
            let mut store = RootCertStore::empty();
            store.add_parsable_certificates(rustls_native_certs::load_native_certs().certs);
            Arc::new(store)
        }
    };
    let client = rustls::ClientConfig::builder_with_provider(Arc::clone(&provider))
        .with_safe_default_protocol_versions()
        .map_err(|e| format!("client: {e}"))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(client));
    let mut destinations: Vec<Arc<Destination>> = Vec::with_capacity(c.upstreams.len());
    let mut ca_pem = Vec::new();
    for u in &c.upstreams {
        let (d, pem) = new_destination(u, &provider, &connector, (c.now)())?;
        // A program names a connection by host and port alone, so two with the same pair could not be told apart.
        if destinations.iter().any(|seen| seen.target == d.target) {
            return Err(format!(
                "two upstreams are {}: a run reaches each host and port through one connection",
                d.target
            ));
        }
        destinations.push(Arc::new(d));
        ca_pem.extend_from_slice(&pem);
    }
    Ok((
        Injection {
            destinations,
            now: Arc::clone(&c.now),
            max_requests: c.max_requests,
            requests: AtomicU64::new(0),
            budget: Budget::new(c.max_bytes),
        },
        ca_pem,
    ))
}

/// Checks one upstream and makes what serves it: its rules, its credential headers and the certificate a program is shown
/// for its host, with the authority (PEM) that signed it.
fn new_destination(
    u: &Upstream,
    provider: &Arc<rustls::crypto::CryptoProvider>,
    connector: &TlsConnector,
    now: SystemTime,
) -> Result<(Destination, Vec<u8>), String> {
    let not_an_address = || format!("the address {:?} is not an http or https address", showable(&u.base));
    let base: Uri = u.base.parse().map_err(|_| not_an_address())?;
    let scheme = base.scheme_str().ok_or_else(not_an_address)?;
    let authority = base.authority().ok_or_else(not_an_address)?.as_str().to_string();
    if scheme != "http" && scheme != "https" {
        return Err(not_an_address());
    }
    if authority.contains('@') || base.query().is_some() || u.base.contains('#') {
        return Err(format!(
            "the address {:?} holds a login, a query or a fragment: it is a scheme, a host and a path",
            showable(&u.base)
        ));
    }
    if u.headers.is_empty() && u.sign.is_none() {
        return Err(format!(
            "{}: no credential header and no signer: a connection that injects nothing is a plain gate's",
            showable(&u.base)
        ));
    }
    if u.max_request_body == 0 || u.max_answer == 0 {
        return Err("max request body and max answer must be positive".to_string());
    }
    let mut headers = Vec::with_capacity(u.headers.len());
    for (name, value) in &u.headers {
        let bad = || format!("credential header {:?} is not a header", showable(name));
        if name.is_empty() || name.contains([' ', '\t', '\r', '\n', ':']) || value.contains(['\r', '\n', '\0']) {
            return Err(bad());
        }
        headers.push((
            HeaderName::from_bytes(name.as_bytes()).map_err(|_| bad())?,
            HeaderValue::from_str(value).map_err(|_| bad())?,
        ));
    }
    let default_port = if scheme == "https" { "443" } else { "80" };
    let (host, port) = split_host_port(&authority).unwrap_or((authority.trim_matches(['[', ']']), default_port));
    let target = new_target(host, port).map_err(|e| format!("the address: {e}"))?;
    let rules = RequestRules::new(base.path(), &u.read_posts, u.writes).map_err(|e| e.to_string())?;

    let identity = server_identity(&target.host, now)?;
    let mut server = rustls::ServerConfig::builder_with_provider(Arc::clone(provider))
        .with_safe_default_protocol_versions()
        .and_then(|b| {
            b.with_no_client_auth()
                .with_single_cert(vec![identity.cert], identity.key)
        })
        .map_err(|e| format!("server identity: {e}"))?;
    server.alpn_protocols = vec![b"http/1.1".to_vec()];
    let server_name =
        rustls_pki_types::ServerName::try_from(host.to_string()).map_err(|e| format!("the address: {e}"))?;
    Ok((
        Destination {
            https: scheme == "https",
            base_host: authority,
            server_name,
            target,
            rules,
            headers,
            sign: u.sign.clone(),
            scrub: Arc::clone(&u.scrub),
            acceptor: TlsAcceptor::from(Arc::new(server)),
            connector: connector.clone(),
            max_request_body: u.max_request_body,
            max_answer: u.max_answer,
        },
        identity.ca_pem,
    ))
}

impl Injection {
    /// The destination a CONNECT names.
    fn tunnelled(&self, t: &Target) -> Option<&Arc<Destination>> {
        self.destinations.iter().find(|d| d.target == *t)
    }

    /// The destination a request in absolute form names (what a client sends a proxy for plain HTTP): its host and port,
    /// the scheme's default when none is written.
    fn absolute(&self, uri: &Uri) -> Result<&Arc<Destination>, Refusal> {
        let (Some(authority), Some(scheme)) = (uri.authority(), uri.scheme_str()) else {
            return Err((
                StatusCode::BAD_REQUEST,
                "this is a forward proxy: send an absolute URI, or CONNECT to an https connection".to_string(),
            ));
        };
        let port = match scheme {
            "http" => 80,
            "https" => 443,
            _ => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "the request is not for http or https".to_string(),
                ));
            }
        };
        let Some(d) = self
            .destinations
            .iter()
            .find(|d| same_authority(authority.as_str(), &d.target, port))
        else {
            return Err((
                StatusCode::FORBIDDEN,
                format!("{} is not on the allowlist", showable(authority.as_str())),
            ));
        };
        if d.https {
            return Err((
                StatusCode::FORBIDDEN,
                "this connection is https: connect with CONNECT and speak TLS".to_string(),
            ));
        }
        Ok(d)
    }
}

impl Destination {
    /// The port of the upstream's scheme.
    pub(crate) fn default_port(&self) -> u16 {
        if self.https { 443 } else { 80 }
    }

    pub(crate) fn scheme(&self) -> &'static str {
        if self.https { "https" } else { "http" }
    }

    /// The address a judged target is asked at.
    pub(crate) fn upstream_url(&self, target: &str) -> Result<Uri, String> {
        let u: Uri = format!("{}://{}{target}", self.scheme(), self.base_host)
            .parse()
            .map_err(|_| "the target does not parse as a path under the connection's address".to_string())?;
        if u.authority().map(|a| a.as_str()) != Some(self.base_host.as_str()) {
            return Err("the target does not parse as a path under the connection's address".to_string());
        }
        Ok(u)
    }

    /// Opens a connection to the upstream: TLS when the address is https.
    pub(crate) async fn open(
        &self,
        tracker: tokio_util::task::TaskTracker,
        tcp: tokio::net::TcpStream,
        req: Request<http_body_util::Full<bytes::Bytes>>,
    ) -> Result<Response<Incoming>, String> {
        let failed = |e: &dyn std::fmt::Display| e.to_string();
        if self.https {
            let tls = self
                .connector
                .connect(self.server_name.clone(), tcp)
                .await
                .map_err(|e| failed(&e))?;
            crate::forward::send(tracker, TokioIo::new(tls), req)
                .await
                .map_err(|e| failed(&e))
        } else {
            crate::forward::send(tracker, TokioIo::new(tcp), req)
                .await
                .map_err(|e| failed(&e))
        }
    }
}

/// What the requests of a connection whose TLS the Injector ended share: the CONNECT's deadline and the destination it
/// named.
#[derive(Clone)]
pub(crate) struct Tunnelled {
    pub(crate) deadline: Instant,
    pub(crate) destination: Arc<Destination>,
}

impl Inner {
    fn injection(&self) -> Option<&Injection> {
        self.inject.as_ref()
    }

    /// serve for an Injector: a CONNECT to an https upstream of the run is answered by ending its TLS here, a request in
    /// absolute form for a plain-HTTP upstream is served as it is, and nothing else is.
    pub(crate) async fn serve_injected(
        self: Arc<Self>,
        mut req: Request<Incoming>,
        deadline: Instant,
        turn: Turn,
    ) -> Response<Body> {
        let Some(inj) = self.injection() else {
            return self.refuse("", StatusCode::INTERNAL_SERVER_ERROR, "the gate does not inject");
        };
        let host = req.uri().authority().map(|a| a.to_string()).unwrap_or_default();
        if req.method() != Method::CONNECT {
            let destination = match inj.absolute(req.uri()) {
                Ok(d) => Arc::clone(d),
                Err((status, reason)) => return self.refuse(&host, status, &reason),
            };
            let gate = Arc::clone(&self);
            return match tokio::time::timeout_at(deadline, gate.exchange(&destination, req)).await {
                Ok(resp) => resp,
                Err(_) => self.refuse(&host, StatusCode::BAD_GATEWAY, "the deadline passed"),
            };
        }
        let t = match parse_target(&host) {
            Ok(t) => t,
            Err(e) => return self.refuse(&host, StatusCode::FORBIDDEN, &format!("CONNECT target: {e}")),
        };
        let Some(destination) = inj.tunnelled(&t) else {
            return self.refuse(
                &t.to_string(),
                StatusCode::FORBIDDEN,
                &format!("{t} is not on the allowlist"),
            );
        };
        if !destination.https {
            return self.refuse(
                &t.to_string(),
                StatusCode::FORBIDDEN,
                "this connection is plain http: send the request as an absolute URI, not CONNECT",
            );
        }
        let upgrade = hyper::upgrade::on(&mut req);
        let inside = Tunnelled {
            deadline,
            destination: Arc::clone(destination),
        };
        let gate = Arc::clone(&self);
        self.tracker.spawn(gate.terminate(upgrade, inside, turn));
        Response::new(Empty::new().map_err(|never| match never {}).boxed())
    }

    /// Serves HTTP over TLS on the upgraded connection, with the certificate made for its destination in this run, until
    /// the program ends it, the deadline passes or the Gate closes.
    ///
    /// Its future is a named, boxed type: it serves a connection, whose requests reach this function again, and the
    /// compiler cannot infer the type of such a cycle.
    fn terminate(
        self: Arc<Self>,
        upgrade: hyper::upgrade::OnUpgrade,
        inside: Tunnelled,
        turn: Turn,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async move {
            let _turn = turn;
            let deadline = inside.deadline;
            let served = async {
                let Ok(client) = upgrade.await else { return };
                let Ok(tls) = inside.destination.acceptor.accept(TokioIo::new(client)).await else {
                    return;
                };
                Arc::clone(&self).serve_connection(tls, Some(inside.clone())).await;
            };
            tokio::select! {
                () = self.token.cancelled() => {}
                _ = tokio::time::timeout_at(deadline, served) => {}
            }
        })
    }
}
