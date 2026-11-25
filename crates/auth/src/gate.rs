//! Who a credential is: the owner key (ADR-0022), or a Google sign-in this server minted for an account on the owner's
//! list (ADR-0105). Both are the one owner; the gate names which way he came in (`local`, or his Google address), so
//! every act is recorded with it.

use crate::google::{self, Firebase, Refused, SigningKeys};
use crate::local::{Error, LocalKey};
use crate::session::Sessions;

/// Lets the owner in, by his key or by Google when the server is set up for it.
pub struct OwnerGate {
    key: LocalKey,
    google: Option<Google>,
}

struct Google {
    firebase: Firebase,
    sessions: Sessions,
}

/// What a Google sign-in hands the browser: the credential every session of it carries, and the account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedIn {
    pub credential: String,
    pub email: String,
}

impl OwnerGate {
    /// The gate of the owner key `key`, with Google sign-in through `firebase` when it is set up.
    pub fn new(key: LocalKey, firebase: Option<Firebase>) -> Result<Self, Error> {
        let google = match firebase {
            Some(firebase) => Some(Google {
                sessions: Sessions::of(key.key()).ok_or(Error::Invalid)?,
                firebase,
            }),
            None => None,
        };
        Ok(Self { key, google })
    }

    /// The owner key, for the link the server prints.
    pub fn key(&self) -> &LocalKey {
        &self.key
    }

    /// The Firebase project the owner signs in through; none: Google sign-in is off.
    pub fn firebase(&self) -> Option<&Firebase> {
        self.google.as_ref().map(|g| &g.firebase)
    }

    /// Who `credential` is at `now` (seconds since the Unix epoch): `local` for the owner key, the Google address for a
    /// sign-in this server minted while that address is still on the list; `Error::Invalid` for anything else.
    pub fn verify(&self, credential: &str, now: i64) -> Result<String, Error> {
        if let Ok(who) = self.key.verify(credential) {
            return Ok(who.to_owned());
        }
        self.google
            .as_ref()
            .and_then(|g| {
                g.sessions
                    .verify(credential, now)
                    .filter(|email| g.firebase.allows(email))
            })
            .ok_or(Error::Invalid)
    }

    /// Signs the owner in with the Firebase ID token `id_token`, checked against Google's `keys` at `now`: the
    /// credential his browser keeps, or why he may not come in. With Google sign-in off nobody comes in this way.
    pub fn sign_in(&self, id_token: &str, keys: &SigningKeys, now: i64) -> Result<SignedIn, Refused> {
        let g = self.google.as_ref().ok_or(Refused::Off)?;
        let email = google::verify_id_token(id_token, keys, &g.firebase, now)?;
        let credential = g.sessions.mint(&email, now).ok_or(Refused::Malformed)?;
        Ok(SignedIn { credential, email })
    }
}
