//! The credential this server hands a browser whose owner signed in with Google (ADR-0105): which account, until when,
//! and a MAC over both with a key drawn from the owner key, so it holds only on the server that made it and only while
//! that owner key is the one in its file. It needs no record: deleting the owner key ends every one of them, as it ends
//! the key's own links. `gs1.<hex of "<expires>:<email>">.<hex of the MAC>`.

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// How long a Google sign-in lasts before the owner signs in with Google again, in seconds: two weeks, as long as
/// Firebase's own session cookies may last.
pub const SESSION_FOR: i64 = 14 * 24 * 60 * 60;

/// What every such credential starts with, naming its format.
const PREFIX: &str = "gs1.";

/// The longest credential looked at: the format with a long email address fits well within it.
const MAX_SESSION: usize = 1024;

/// What the MAC key is drawn from the owner key with, so this MAC never serves another purpose.
const PURPOSE: &[u8] = b"remit: google sign-in session, gs1";

/// Mints and checks the Google sessions of one server.
pub struct Sessions {
    key: [u8; 32],
}

impl Sessions {
    /// The sessions keyed by the owner key `owner_key`; none if HMAC refused the key, which it does for no length.
    pub(crate) fn of(owner_key: &str) -> Option<Self> {
        Some(Self {
            key: mac(owner_key.as_bytes(), &[PURPOSE])?,
        })
    }

    /// A credential for `email`, made at `now` (seconds since the Unix epoch), that holds for `SESSION_FOR`.
    pub fn mint(&self, email: &str, now: i64) -> Option<String> {
        let payload = hex::encode(format!("{}:{email}", now.saturating_add(SESSION_FOR)));
        let tag = hex::encode(mac(&self.key, &[PREFIX.as_bytes(), payload.as_bytes()])?);
        Some(format!("{PREFIX}{payload}.{tag}"))
    }

    /// The account `credential` was minted for, while it holds at `now`; none for anything else.
    pub fn verify(&self, credential: &str, now: i64) -> Option<String> {
        if credential.len() > MAX_SESSION {
            return None;
        }
        let (payload, tag) = credential.strip_prefix(PREFIX)?.split_once('.')?;
        let tag = hex::decode(tag).ok()?;
        let mut m = HmacSha256::new_from_slice(&self.key).ok()?;
        m.update(PREFIX.as_bytes());
        m.update(payload.as_bytes());
        // Constant time: a forger learns nothing from how long the check took.
        m.verify_slice(&tag).ok()?;
        let payload = String::from_utf8(hex::decode(payload).ok()?).ok()?;
        let (expires, email) = payload.split_once(':')?;
        let expires: i64 = expires.parse().ok()?;
        (now < expires && !email.is_empty()).then(|| email.to_owned())
    }
}

/// HMAC-SHA256 of the parts, in order, under `key`; none if HMAC refused the key, which it does for no length.
fn mac(key: &[u8], parts: &[&[u8]]) -> Option<[u8; 32]> {
    let mut m = HmacSha256::new_from_slice(key).ok()?;
    for p in parts {
        m.update(p);
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&m.finalize().into_bytes());
    Some(out)
}
