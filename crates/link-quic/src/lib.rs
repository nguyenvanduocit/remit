//! The link over WebTransport on QUIC (ADR-0072 decision 3): `link::Conn` driven by `web-transport-quinn` on tokio, for
//! the server (`Listener`) and every native peer (`dial`). One WebTransport session per pair of peers, at `/link`; each
//! session runs one task that owns its `Conn`, and each stream one task that reads it and writes what the `Conn` hands it.

mod driver;
mod stream;
mod streams;

use std::fmt;
use std::net::SocketAddr;

use link::{Config, Conn, Hello, Node, Side};
use web_transport_quinn::http::StatusCode;
use web_transport_quinn::{ClientBuilder, ClientError, ServerBuilder, ServerError};

pub use driver::{Link, Peer};
pub use streams::{Happened, Streams};
pub use url::Url;
pub use web_transport_quinn::Client;
pub use web_transport_quinn::quinn::rustls::pki_types::{CertificateDer, PrivateKeyDer};

/// The one path every peer's session is opened at.
pub const PATH: &str = "/link";

#[derive(Debug)]
pub enum Error {
    Client(ClientError),
    Server(ServerError),
    /// A session was asked for at another path than `PATH`; it was turned away with 404.
    Path(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Client(e) => write!(f, "dial: {e}"),
            Self::Server(e) => write!(f, "accept: {e}"),
            Self::Path(path) => write!(f, "a session was asked for at {path}, not {PATH}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Client(e) => Some(e),
            Self::Server(e) => Some(e),
            Self::Path(_) => None,
        }
    }
}

/// Which servers a dialling peer believes: a certificate from a root the system trusts (production), or one whose
/// SHA-256 hash it was handed (the development server's short-lived self-signed certificate, ADR-0072 decision 3).
#[derive(Debug, Clone)]
pub enum Trust {
    SystemRoots,
    CertificateHashes(Vec<Vec<u8>>),
}

/// A client that dials many sessions; it needs a tokio runtime to be made.
pub fn client(trust: Trust) -> Result<Client, Error> {
    let builder = ClientBuilder::new();
    match trust {
        Trust::SystemRoots => builder.with_system_roots(),
        Trust::CertificateHashes(hashes) => builder.with_server_certificate_hashes(hashes),
    }
    .map_err(Error::Client)
}

/// Opens a session to `url` (its path is `PATH`) and says hello on it; `Event::Welcomed` or `Event::Refused` follows on
/// the peer's events.
pub async fn dial(client: &Client, url: Url, cfg: Config, hello: Hello) -> Result<Peer, Error> {
    let session = client.connect(url).await.map_err(Error::Client)?;
    Ok(driver::start(session, Conn::dial(cfg, hello), Side::Acceptor))
}

/// Where peers' sessions arrive.
pub struct Listener {
    server: web_transport_quinn::Server,
    cfg: Config,
    me: Node,
}

impl Listener {
    /// Listens on `addr` with this certificate; needs a tokio runtime.
    pub fn bind(
        addr: SocketAddr,
        chain: Vec<CertificateDer<'static>>,
        key: PrivateKeyDer<'static>,
        cfg: Config,
        me: Node,
    ) -> Result<Self, Error> {
        let server = ServerBuilder::new()
            .with_addr(addr)
            .with_certificate(chain, key)
            .map_err(Error::Server)?;
        Ok(Self { server, cfg, me })
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.server.local_addr()
    }

    /// The next peer's session, once it is open; its `Event::Hello` comes first on its events, to `admit` or `refuse`.
    /// None once the listener is closed.
    pub async fn accept(&mut self) -> Option<Result<Peer, Error>> {
        let request = self.server.accept().await?;
        let path = request.url.path().to_owned();
        if path != PATH {
            return Some(match request.reject(StatusCode::NOT_FOUND).await {
                Ok(()) => Err(Error::Path(path)),
                Err(e) => Err(Error::Server(e)),
            });
        }
        Some(match request.ok().await {
            Ok(session) => Ok(driver::start(
                session,
                Conn::accept(self.cfg, self.me.clone()),
                Side::Dialer,
            )),
            Err(e) => Err(Error::Server(e)),
        })
    }
}
