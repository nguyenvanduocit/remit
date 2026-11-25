//! Who this server is on the link (ADR-0072 decisions 3 and 12): its node, whose Ed25519 key is made at the first start in
//! the data directory and kept owner-only (recorded in `hello`/`welcome`, not yet proven: *Revisit when* a second node
//! joins), and the certificate its WebTransport listener presents, made at each start, self-signed ECDSA P-256 and valid
//! less than two weeks, so a browser and the AI runtime accept it by its SHA-256 hash.

use std::fs::OpenOptions;
use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::Path;

use link_quic::{CertificateDer, PrivateKeyDer};
use ring::rand::SystemRandom;
use ring::signature::{Ed25519KeyPair, KeyPair as _};

/// How long the start's certificate is valid: a browser accepts a certificate by its hash only when it is valid for less
/// than two weeks.
const CERTIFICATE_DAYS: i64 = 13;

/// This node, from its key at path, made there owner-only when there is none.
pub fn node(path: &Path) -> Result<link::Node, String> {
    let pkcs8 = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => make_key(path)?,
        Err(e) => return Err(format!("the node key {}: {e}", path.display())),
    };
    let pair = Ed25519KeyPair::from_pkcs8(&pkcs8).map_err(|e| format!("the node key {}: {e}", path.display()))?;
    let public = pair.public_key().as_ref();
    Ok(link::Node {
        id: hex::encode(ring::digest::digest(&ring::digest::SHA256, public)),
        public_key: hex::encode(public),
    })
}

fn make_key(path: &Path) -> Result<Vec<u8>, String> {
    let doc = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).map_err(|e| format!("make the node key: {e}"))?;
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| format!("the node key {}: {e}", path.display()))?;
    f.write_all(doc.as_ref())
        .and_then(|()| f.sync_all())
        .map_err(|e| format!("the node key {}: {e}", path.display()))?;
    Ok(doc.as_ref().to_vec())
}

/// The certificate the link's listener presents this start, its key, its SHA-256 hash and when it expires.
pub struct Certificate {
    pub chain: Vec<CertificateDer<'static>>,
    pub key: PrivateKeyDer<'static>,
    pub hash: Vec<u8>,
    pub not_after: time::OffsetDateTime,
}

/// A certificate for this machine's loopback names, valid from an hour ago for `CERTIFICATE_DAYS`.
pub fn certificate(now: time::OffsetDateTime) -> Result<Certificate, String> {
    let names = vec!["localhost".to_owned(), "127.0.0.1".to_owned(), "::1".to_owned()];
    let mut params = rcgen::CertificateParams::new(names).map_err(|e| format!("the link's certificate: {e}"))?;
    params.not_before = now.saturating_sub(time::Duration::hours(1));
    let not_after = now.saturating_add(time::Duration::days(CERTIFICATE_DAYS));
    params.not_after = not_after;
    let key = rcgen::KeyPair::generate().map_err(|e| format!("the link's certificate key: {e}"))?;
    let cert = params
        .self_signed(&key)
        .map_err(|e| format!("the link's certificate: {e}"))?;
    let der = cert.der().to_vec();
    let hash = ring::digest::digest(&ring::digest::SHA256, &der).as_ref().to_vec();
    Ok(Certificate {
        chain: vec![CertificateDer::from(der)],
        key: PrivateKeyDer::Pkcs8(key.serialize_der().into()),
        hash,
        not_after,
    })
}
