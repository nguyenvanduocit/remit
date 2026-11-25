//! Opening a session on a server's link (ADR-0072 decision 3): its `/link.json` says where the link is and which
//! certificate to believe there, and a hello presents a credential and the contract it asks to call. Every command of
//! the client, and `remit update` and `remit doctor`, come in this way.

use link::{Closing, Config, Event, Hello, Refusal};
use link_quic::{Peer, Trust, Url};
use serde::Deserialize;
use std::time::Duration;

/// How long finding where the link is may take.
const LINK_JSON_WAIT: Duration = Duration::from_secs(15);

/// What the server's `/link.json` says: where its link is, the hashes of the certificate it presents there and when
/// that certificate expires (RFC 3339).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkJson {
    pub url: String,
    #[serde(default)]
    pub certificate_hashes: Vec<String>,
    #[serde(default)]
    pub certificate_expires: Option<String>,
}

/// Why no session is up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Down {
    /// The server turned the hello away.
    Refused(Refusal),
    /// The server, or its link, did not answer.
    Unreachable(String),
}

impl std::fmt::Display for Down {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(r) => write!(f, "refused ({:?}): {}", r.code, r.reason),
            Self::Unreachable(why) => f.write_str(why),
        }
    }
}

/// A provider for reqwest's https and the link's TLS: ring, as every TLS of Remit uses. One installed already is as good.
fn crypto() {
    rustls::crypto::ring::default_provider().install_default().ok();
}

/// What the server at `server` (`http://host:port`, `https://...`) says of its link.
pub async fn link_json(server: &str) -> Result<LinkJson, String> {
    crypto();
    let at = format!("{}/link.json", server.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(LINK_JSON_WAIT)
        .build()
        .map_err(|e| e.to_string())?;
    let res = client.get(&at).send().await.map_err(|e| format!("{at}: {e}"))?;
    if !res.status().is_success() {
        return Err(format!("{at} answered {}", res.status()));
    }
    let body = res.bytes().await.map_err(|e| format!("{at}: {e}"))?;
    serde_json::from_slice(&body).map_err(|e| format!("{at}: {e}"))
}

/// The link's address and the certificate to believe there: the hashes it names (a server's self-signed certificate),
/// else the system's roots. Pure.
pub fn trust_of(link: &LinkJson) -> Result<(Url, Trust), String> {
    let url = Url::parse(&link.url).map_err(|e| format!("the link's address {:?}: {e}", link.url))?;
    if link.certificate_hashes.is_empty() {
        return Ok((url, Trust::SystemRoots));
    }
    let hashes = link
        .certificate_hashes
        .iter()
        .map(|h| hex::decode(h).map_err(|e| format!("a certificate hash {h:?}: {e}")))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((url, Trust::CertificateHashes(hashes)))
}

/// A session at `url` welcomed by the server, or why there is none.
pub async fn open_at(url: Url, trust: Trust, hello: Hello) -> Result<Peer, Down> {
    crypto();
    let client = link_quic::client(trust).map_err(|e| Down::Unreachable(e.to_string()))?;
    let mut peer = link_quic::dial(&client, url, Config::default(), hello)
        .await
        .map_err(|e| Down::Unreachable(e.to_string()))?;
    loop {
        match peer.events.recv().await {
            Some(Event::Welcomed(_)) => return Ok(peer),
            Some(Event::Refused(r) | Event::Closed(Closing::Refused(r))) => return Err(Down::Refused(r)),
            Some(Event::Closed(closing)) => return Err(Down::Unreachable(why_closed(&closing))),
            None => {
                return Err(Down::Unreachable(
                    "the session ended before the server answered".to_owned(),
                ));
            }
            Some(_) => {}
        }
    }
}

/// A session on the link of the server at `server`, welcomed, or why there is none.
pub async fn open(server: &str, hello: Hello) -> Result<Peer, Down> {
    let link = link_json(server).await.map_err(Down::Unreachable)?;
    let (url, trust) = trust_of(&link).map_err(Down::Unreachable)?;
    open_at(url, trust, hello).await
}

/// Why a session closed, in words.
pub fn why_closed(closing: &Closing) -> String {
    match closing {
        Closing::Done => "the connection was lost".to_owned(),
        Closing::Behind => "this client fell behind the server".to_owned(),
        Closing::Broken(why) => why.clone(),
        Closing::Refused(r) => r.reason.clone(),
    }
}
