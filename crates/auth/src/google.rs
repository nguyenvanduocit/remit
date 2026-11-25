//! Checks a Firebase ID token, the proof Google's sign-in hands the browser, as Firebase says a server without its Admin
//! SDK checks one (https://firebase.google.com/docs/auth/admin/verify-id-tokens): RS256, signed by one of Google's
//! securetoken keys, for this project, not expired, then an account signed in with Google whose verified address is on
//! the owner's list (ADR-0105). Sans-IO: the keys and the time are given; fetching the keys is the server's.

use std::collections::{BTreeSet, HashMap};

use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::Deserialize;

/// Where Google publishes the keys Firebase ID tokens are signed with, as X.509 certificates keyed by `kid`; its
/// `Cache-Control: max-age` says how long they hold.
pub const SIGNING_KEYS_URL: &str =
    "https://www.googleapis.com/robot/v1/metadata/x509/securetoken@system.gserviceaccount.com";

/// The longest ID token looked at; a Firebase ID token is around a kilobyte, so anything longer is refused unread.
pub const MAX_ID_TOKEN: usize = 8 * 1024;

/// How far in the future `iat` and `auth_time` may be, in seconds: this machine's clock may run behind Google's by
/// that much, and a token minted a moment ago is not refused for it. `exp` gets no grace.
pub const CLOCK_SKEW: i64 = 60;

/// The sign-in provider of an account that came in with Google.
const GOOGLE_PROVIDER: &str = "google.com";

/// The Firebase project the owner signs in through and the Google accounts that come in as him.
#[derive(Debug, Clone)]
pub struct Firebase {
    project: String,
    allowed: BTreeSet<String>,
}

/// Why a Firebase setting cannot be used.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("the Firebase project id is empty")]
    NoProject,
    #[error("no Google account may come in: name at least one")]
    NoAccounts,
    #[error("{0:?} is not an email address")]
    NotAnEmail(String),
}

impl Firebase {
    /// The project `project` lets in the accounts `emails` (compared without regard to case, as Google's are).
    pub fn new<'a>(project: &str, emails: impl IntoIterator<Item = &'a str>) -> Result<Self, ConfigError> {
        let project = project.trim();
        if project.is_empty() {
            return Err(ConfigError::NoProject);
        }
        let mut allowed = BTreeSet::new();
        for email in emails.into_iter().map(str::trim).filter(|e| !e.is_empty()) {
            if !email.contains('@') {
                return Err(ConfigError::NotAnEmail(email.to_owned()));
            }
            allowed.insert(email.to_ascii_lowercase());
        }
        if allowed.is_empty() {
            return Err(ConfigError::NoAccounts);
        }
        Ok(Self {
            project: project.to_owned(),
            allowed,
        })
    }

    /// The project id, which a token's `aud` names.
    pub fn project(&self) -> &str {
        &self.project
    }

    /// Whether `email` is on the list.
    pub fn allows(&self, email: &str) -> bool {
        self.allowed.contains(&email.to_ascii_lowercase())
    }
}

/// Google's securetoken keys, by `kid`.
pub struct SigningKeys(HashMap<String, DecodingKey>);

/// Why the keys Google published could not be read.
#[derive(Debug, thiserror::Error)]
pub enum KeysError {
    #[error("the signing keys are not a JSON object of PEM texts: {0}")]
    Json(#[from] serde_json::Error),
    #[error("signing key {kid}: {err}")]
    Key {
        kid: String,
        err: jsonwebtoken::errors::Error,
    },
    #[error("Google published no signing key")]
    Empty,
}

impl SigningKeys {
    /// The keys of the endpoint's body: `{"<kid>": "<PEM certificate>", …}`.
    pub fn parse(body: &str) -> Result<Self, KeysError> {
        let pems: HashMap<String, String> = serde_json::from_str(body)?;
        let mut keys = HashMap::with_capacity(pems.len());
        for (kid, pem) in pems {
            match DecodingKey::from_rsa_pem(pem.as_bytes()) {
                Ok(key) => {
                    keys.insert(kid, key);
                }
                Err(err) => return Err(KeysError::Key { kid, err }),
            }
        }
        if keys.is_empty() {
            return Err(KeysError::Empty);
        }
        Ok(Self(keys))
    }
}

/// Why an ID token does not let its holder in. Each says what was wrong with the token, never anything of the keys.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Refused {
    #[error("Google sign-in is not set up on this server")]
    Off,
    #[error("the ID token is longer than {MAX_ID_TOKEN} bytes")]
    TooLong,
    #[error("the ID token is not a JWT signed with RS256")]
    Malformed,
    #[error("the ID token is signed with a key Google does not publish")]
    UnknownKey,
    #[error("the ID token's signature does not hold")]
    BadSignature,
    #[error("the ID token is for another Firebase project")]
    WrongAudience,
    #[error("the ID token was not issued by Firebase for this project")]
    WrongIssuer,
    #[error("the ID token has expired")]
    Expired,
    #[error("the ID token was issued in the future")]
    IssuedInFuture,
    #[error("the ID token names no account")]
    NoSubject,
    #[error("the account did not sign in with Google")]
    NotGoogle,
    #[error("the account's email address is not verified")]
    EmailNotVerified,
    #[error("{0} may not come in")]
    NotAllowed(String),
}

#[derive(Deserialize)]
struct Claims {
    aud: String,
    iss: String,
    #[serde(default)]
    sub: String,
    exp: i64,
    iat: i64,
    auth_time: i64,
    #[serde(default)]
    email: String,
    #[serde(default)]
    email_verified: bool,
    #[serde(default)]
    firebase: SignInClaims,
}

#[derive(Deserialize, Default)]
struct SignInClaims {
    #[serde(default)]
    sign_in_provider: String,
}

/// The owner's Google address `token` proves at `now` (seconds since the Unix epoch), or why it does not let him in.
pub fn verify_id_token(token: &str, keys: &SigningKeys, firebase: &Firebase, now: i64) -> Result<String, Refused> {
    if token.len() > MAX_ID_TOKEN {
        return Err(Refused::TooLong);
    }
    let header = jsonwebtoken::decode_header(token).map_err(|_| Refused::Malformed)?;
    if header.alg != Algorithm::RS256 {
        return Err(Refused::Malformed);
    }
    let kid = header.kid.ok_or(Refused::UnknownKey)?;
    let key = keys.0.get(&kid).ok_or(Refused::UnknownKey)?;
    // The library checks the signature and the algorithm only; every claim is checked below against `now`.
    let mut only_signature = Validation::new(Algorithm::RS256);
    only_signature.validate_exp = false;
    only_signature.validate_aud = false;
    only_signature.required_spec_claims.clear();
    let claims = jsonwebtoken::decode::<Claims>(token, key, &only_signature)
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::InvalidSignature => Refused::BadSignature,
            _ => Refused::Malformed,
        })?
        .claims;
    if claims.aud != firebase.project {
        return Err(Refused::WrongAudience);
    }
    if claims.iss != format!("https://securetoken.google.com/{}", firebase.project) {
        return Err(Refused::WrongIssuer);
    }
    if claims.exp <= now {
        return Err(Refused::Expired);
    }
    let latest = now.saturating_add(CLOCK_SKEW);
    if claims.iat > latest || claims.auth_time > latest {
        return Err(Refused::IssuedInFuture);
    }
    if claims.sub.is_empty() {
        return Err(Refused::NoSubject);
    }
    if claims.firebase.sign_in_provider != GOOGLE_PROVIDER {
        return Err(Refused::NotGoogle);
    }
    if !claims.email_verified || claims.email.is_empty() {
        return Err(Refused::EmailNotVerified);
    }
    if !firebase.allows(&claims.email) {
        return Err(Refused::NotAllowed(claims.email));
    }
    Ok(claims.email.to_ascii_lowercase())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use jsonwebtoken::{EncodingKey, Header};
    use rsa::pkcs1::EncodeRsaPrivateKey;
    use rsa::pkcs8::{EncodePublicKey, LineEnding};
    use serde_json::{Value, json};
    use std::sync::OnceLock;

    const PROJECT: &str = "remit-test";
    const NOW: i64 = 1_800_000_000;
    const BEFORE: i64 = 1_799_999_000;
    const AFTER: i64 = 1_800_003_600;
    const FAR_AHEAD: i64 = 1_800_000_600;

    /// One RSA key for every test, made once: making one is the slow part.
    pub(crate) fn signer() -> &'static (EncodingKey, SigningKeys) {
        static SIGNER: OnceLock<(EncodingKey, SigningKeys)> = OnceLock::new();
        SIGNER.get_or_init(|| {
            let key = rsa::RsaPrivateKey::new(&mut rand::thread_rng(), 2048).unwrap();
            let private = key.to_pkcs1_pem(LineEnding::LF).unwrap();
            let public = rsa::RsaPublicKey::from(&key).to_public_key_pem(LineEnding::LF).unwrap();
            let published = json!({ "k1": public }).to_string();
            (
                EncodingKey::from_rsa_pem(private.as_bytes()).unwrap(),
                SigningKeys::parse(&published).unwrap(),
            )
        })
    }

    fn firebase() -> Firebase {
        Firebase::new(PROJECT, ["owner@example.com"]).unwrap()
    }

    /// What Firebase puts in the ID token of an account that signed in with Google a moment ago.
    pub(crate) fn good() -> Value {
        json!({
            "aud": PROJECT,
            "iss": format!("https://securetoken.google.com/{PROJECT}"),
            "sub": "uid-1",
            "iat": BEFORE,
            "auth_time": BEFORE,
            "exp": AFTER,
            "email": "Owner@Example.com",
            "email_verified": true,
            "firebase": { "sign_in_provider": "google.com", "identities": {} },
        })
    }

    fn with(field: &str, value: Value) -> Value {
        let mut claims = good();
        claims[field] = value;
        claims
    }

    pub(crate) fn sign(claims: &Value) -> String {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("k1".to_owned());
        jsonwebtoken::encode(&header, claims, &signer().0).unwrap()
    }

    fn check(token: &str) -> Result<String, Refused> {
        verify_id_token(token, &signer().1, &firebase(), NOW)
    }

    #[test]
    fn a_token_that_does_not_hold_is_refused_for_what_is_wrong_with_it() {
        let mut forged = sign(&good());
        let other = sign(&with("email", json!("someone@example.com")));
        // The signature of one token over the claims of another.
        let claims_of = |t: &str| t.split('.').nth(1).unwrap().to_owned();
        forged = forged.replacen(&claims_of(&forged), &claims_of(&other), 1);
        let mut other_key = Header::new(Algorithm::RS256);
        other_key.kid = Some("k2".to_owned());
        let unknown = jsonwebtoken::encode(&other_key, &good(), &signer().0).unwrap();
        let cases = [
            (
                "another project",
                sign(&with("aud", json!("someone-else"))),
                Refused::WrongAudience,
            ),
            (
                "another issuer",
                sign(&with("iss", json!("https://securetoken.google.com/someone-else"))),
                Refused::WrongIssuer,
            ),
            ("expired", sign(&with("exp", json!(NOW))), Refused::Expired),
            (
                "issued ahead",
                sign(&with("iat", json!(FAR_AHEAD))),
                Refused::IssuedInFuture,
            ),
            (
                "signed in ahead",
                sign(&with("auth_time", json!(FAR_AHEAD))),
                Refused::IssuedInFuture,
            ),
            ("no account", sign(&with("sub", json!(""))), Refused::NoSubject),
            (
                "another provider",
                sign(&with("firebase", json!({ "sign_in_provider": "password" }))),
                Refused::NotGoogle,
            ),
            (
                "unverified email",
                sign(&with("email_verified", json!(false))),
                Refused::EmailNotVerified,
            ),
            (
                "off the list",
                sign(&with("email", json!("someone@example.com"))),
                Refused::NotAllowed("someone@example.com".to_owned()),
            ),
            ("bad signature", forged, Refused::BadSignature),
            ("unknown key", unknown, Refused::UnknownKey),
            ("not a JWT", "owner".to_owned(), Refused::Malformed),
            ("too long", "a".repeat(MAX_ID_TOKEN.saturating_add(1)), Refused::TooLong),
        ];
        for (name, token, want) in cases {
            assert_eq!(check(&token), Err(want), "{name}");
        }
    }
}
