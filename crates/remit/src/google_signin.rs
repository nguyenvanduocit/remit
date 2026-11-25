//! The owner's Google sign-in, the server's side of it (ADR-0105): `GET /auth/config` tells the way in whether Google is
//! on and which Firebase project to set up; `POST /auth/google` takes the ID token Google's sign-in handed the page,
//! has `auth::OwnerGate` check it against Google's keys and answers the credential the browser keeps. Who comes in is
//! the gate's to say; this fetches the keys, keeps them as long as Google says they hold, and speaks HTTP.

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use wire::app::{FirebaseWeb, GoogleSignIn, GoogleSignedIn, SignInConfig};

use crate::flags::Flags;

/// The largest body `/auth/google` reads: an ID token is around a kilobyte (`auth::google::MAX_ID_TOKEN` bounds it).
const MAX_BODY: usize = 16 * 1024;

/// How long Google's keys are kept when its answer says nothing of it.
const KEEP_KEYS: Duration = Duration::from_secs(5 * 60);

/// How long fetching Google's keys may take.
const FETCH_FOR: Duration = Duration::from_secs(10);

/// Google sign-in as the flags set it up: the project and accounts the gate checks, and what the page sets Firebase up
/// with.
pub struct Setup {
    pub firebase: auth::google::Firebase,
    pub web: FirebaseWeb,
}

/// Google sign-in as -firebase-project, -firebase-api-key, -firebase-auth-domain and -google-emails set it up; none
/// when all are empty. Half of them set is refused at start, as a way in that is half set up is no way in.
pub fn setup(f: &Flags) -> Result<Option<Setup>, String> {
    let project = f.firebase_project.trim();
    let api_key = f.firebase_api_key.trim();
    let domain = f.firebase_auth_domain.trim();
    let emails = f.google_emails.trim();
    if [project, api_key, domain, emails].iter().all(|v| v.is_empty()) {
        return Ok(None);
    }
    if project.is_empty() || api_key.is_empty() || emails.is_empty() {
        return Err(
            "Google sign-in needs -firebase-project, -firebase-api-key and -google-emails together (ADR-0105)"
                .to_owned(),
        );
    }
    let firebase = auth::google::Firebase::new(project, emails.split(','))
        .map_err(|e| format!("-google-emails, -firebase-project: {e}"))?;
    let auth_domain = if domain.is_empty() {
        format!("{project}.firebaseapp.com")
    } else {
        domain.to_owned()
    };
    Ok(Some(Setup {
        firebase,
        web: FirebaseWeb {
            api_key: api_key.to_owned(),
            auth_domain,
            project_id: project.to_owned(),
        },
    }))
}

#[derive(Clone)]
struct SignIn {
    gate: Arc<auth::OwnerGate>,
    web: Option<FirebaseWeb>,
    keys: Arc<KeyCache>,
}

/// The two routes of the way in; `web` is none when Google sign-in is off.
pub fn router(gate: Arc<auth::OwnerGate>, web: Option<FirebaseWeb>) -> Result<Router, String> {
    let client = reqwest::Client::builder()
        .timeout(FETCH_FOR)
        .build()
        .map_err(|e| format!("the client Google's keys are fetched with: {e}"))?;
    let state = SignIn {
        gate,
        web,
        keys: Arc::new(KeyCache {
            client,
            held: tokio::sync::Mutex::new(None),
        }),
    };
    Ok(Router::new()
        .route("/auth/config", get(config))
        .route("/auth/google", post(sign_in).layer(DefaultBodyLimit::max(MAX_BODY)))
        .with_state(state))
}

async fn config(State(s): State<SignIn>) -> Response {
    crate::serve::json(StatusCode::OK, &SignInConfig { google: s.web })
}

async fn sign_in(State(s): State<SignIn>, body: Bytes) -> Response {
    if s.web.is_none() {
        return (StatusCode::NOT_FOUND, auth::google::Refused::Off.to_string()).into_response();
    }
    let Ok(asked) = serde_json::from_slice::<GoogleSignIn>(&body) else {
        return (StatusCode::BAD_REQUEST, "the body is not {\"idToken\": \"…\"}").into_response();
    };
    let keys = match s.keys.get().await {
        Ok(keys) => keys,
        Err(err) => {
            tracing::error!(%err, "Google's signing keys not fetched");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "the server could not reach Google to check the sign-in: try again",
            )
                .into_response();
        }
    };
    match s.gate.sign_in(&asked.id_token, &keys, unix_now()) {
        Ok(auth::SignedIn { credential, email }) => {
            tracing::info!(email, "the owner signed in with Google");
            crate::serve::json(StatusCode::OK, &GoogleSignedIn { credential, email })
        }
        Err(why) => {
            tracing::warn!(%why, "a Google sign-in refused");
            (StatusCode::FORBIDDEN, why.to_string()).into_response()
        }
    }
}

/// Seconds since the Unix epoch, by this machine's clock.
fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// Google's signing keys, fetched when first needed and again once they stop holding.
struct KeyCache {
    client: reqwest::Client,
    held: tokio::sync::Mutex<Option<Held>>,
}

struct Held {
    keys: Arc<auth::google::SigningKeys>,
    until: Option<Instant>,
}

impl KeyCache {
    /// The keys, fetched if those held have stopped holding. One fetch at a time: sign-ins meanwhile wait for it.
    async fn get(&self) -> Result<Arc<auth::google::SigningKeys>, String> {
        let mut held = self.held.lock().await;
        if let Some(h) = held.as_ref().filter(|h| h.until.is_some_and(|u| Instant::now() < u)) {
            return Ok(Arc::clone(&h.keys));
        }
        let res = self
            .client
            .get(auth::google::SIGNING_KEYS_URL)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = res.status();
        if !status.is_success() {
            return Err(format!("Google answered {status}"));
        }
        let keep = res
            .headers()
            .get(reqwest::header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok())
            .and_then(max_age)
            .map_or(KEEP_KEYS, Duration::from_secs);
        let body = res.bytes().await.map_err(|e| e.to_string())?;
        let body = std::str::from_utf8(&body).map_err(|e| e.to_string())?;
        let keys = Arc::new(auth::google::SigningKeys::parse(body).map_err(|e| e.to_string())?);
        *held = Some(Held {
            keys: Arc::clone(&keys),
            until: Instant::now().checked_add(keep),
        });
        Ok(keys)
    }
}

/// The `max-age` of a Cache-Control header, in seconds.
fn max_age(cache_control: &str) -> Option<u64> {
    cache_control
        .split(',')
        .find_map(|d| d.trim().strip_prefix("max-age="))
        .and_then(|v| v.trim().parse().ok())
}
