//! The door outside systems post to (GitLab, GitHub, Sentry, Jira, a script through `remit emit`, anything the owner points
//! at it): a channel like the browser's socket, thin on purpose, and one of the writers of the event signals (ADR-0024). A
//! request is let in only through the door of an event signal the owner made, by the name in its address, and only with that
//! signal's own key, or its body signed with it. The door takes the key out of what it passes on and hands the rest, whatever
//! its shape, to Remit ([`Emitter::emit`]), which keeps it as an event and gives it to agents. It asks nothing of the body:
//! what an event means is an agent's to read.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use axum::Router;
use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::header::{CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS};
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use model::{EventSignal, EventVia, Time, Writer, WriterKind};
use orch::{Clock, Emission, Emitted, KeyProof, Orchestrator, Signed};

/// Where the door is mounted: the last part of the path is the name of the event signal the post is an event of.
pub const PATTERN: &str = "/hooks/{name}";

/// How much of a request is read: a sender that sends more is refused, so a request never fills the memory. What Remit
/// keeps of a body is its own limit (orch's `EVENT_BODY_MAX`).
const MAX_BODY: usize = 1 << 20;

/// The header by which a client of the door says which way the event came in: `remit emit` sets it to cli. It names a
/// channel and grants nothing.
pub const VIA_HEADER: &str = "X-Remit-Via";

/// The headers by which a sender numbers what it posts, so that its retry is the same event.
const SENDER_ID_HEADERS: [&str; 3] = ["X-Gitlab-Event-UUID", "X-GitHub-Delivery", "Idempotency-Key"];

/// What in the name of a header says it may carry a credential: the key the sender put in one, another system's key, token
/// or signature, or a login that has no business here. What an event keeps reaches agents.
const CREDENTIAL_WORDS: [&str; 6] = ["token", "secret", "key", "auth", "signature", "cookie"];

/// The headers the HTTP layer reads for itself, which are no part of what the sender says.
const TRANSPORT_HEADERS: [&str; 2] = ["host", "transfer-encoding"];

/// How far a signed stamp may be from now, either way, in seconds: a post signed earlier is a replay of one an eavesdropper
/// kept.
const STAMP_TOLERANCE_SECS: i128 = 5 * 60;

/// Whether a proof of a key opens the door of an event signal: it exists, is not paused and the key is its own. An error is
/// the server's.
pub trait Keys: Send + Sync + 'static {
    fn authorize_event_signal(
        &self,
        name: &str,
        proof: &KeyProof,
    ) -> impl Future<Output = orch::Result<Option<EventSignal>>> + Send;
}

/// What the door needs of Remit to keep a post: the one write path of an event.
pub trait Emitter: Send + Sync + 'static {
    fn emit(&self, e: Emission) -> impl Future<Output = orch::Result<Emitted>> + Send;
}

impl Keys for Orchestrator {
    fn authorize_event_signal(
        &self,
        name: &str,
        proof: &KeyProof,
    ) -> impl Future<Output = orch::Result<Option<EventSignal>>> + Send {
        self.pipeline.authorize_event_signal(name, proof)
    }
}

impl Emitter for Orchestrator {
    fn emit(&self, e: Emission) -> impl Future<Output = orch::Result<Emitted>> + Send {
        self.pipeline.emit(e)
    }
}

struct Door<K, E> {
    keys: Arc<K>,
    to: Arc<E>,
    /// The time a signed stamp is held to.
    now: Clock,
}

/// The door, routed at [`PATTERN`] for POST; now is the clock the stamps of the senders that sign one are held to.
pub fn router<K: Keys, E: Emitter>(keys: Arc<K>, to: Arc<E>, now: Clock) -> Router {
    Router::new()
        .route(PATTERN, post(open::<K, E>))
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .with_state(Arc::new(Door { keys, to, now }))
}

/// Takes in one post. The body is read first, within `MAX_BODY`, since a signature is a proof only of the body it signs.
async fn open<K: Keys, E: Emitter>(
    State(door): State<Arc<Door<K, E>>>,
    Path(name): Path<String>,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let body = match body {
        Ok(body) => body,
        Err(rejection) if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE => {
            return refuse(StatusCode::PAYLOAD_TOO_LARGE, "the body is too large");
        }
        Err(_) => return refuse(StatusCode::BAD_REQUEST, "the body could not be read"),
    };
    let query = query_of(&uri);
    let proof = proof_of(&headers, &query, &body, (door.now)());
    let es = match door.keys.authorize_event_signal(&name, &proof).await {
        Ok(Some(es)) => es,
        // the same answer for a name nobody made, a paused signal and a wrong key
        Ok(None) => return refuse(StatusCode::UNAUTHORIZED, "unauthorized"),
        Err(err) => {
            tracing::error!(signal = %name, %err, "event signal key not checked");
            return refuse(StatusCode::INTERNAL_SERVER_ERROR, "the key could not be checked");
        }
    };
    if let Some(challenge) = challenge_in(&body) {
        // the sender proves the address is the owner's: it is no news
        return ([(CONTENT_TYPE, "text/plain")], challenge).into_response();
    }
    let emission = Emission {
        signal: es.name.clone(),
        writer: writer_of(&headers),
        header: headers_of(&headers),
        query: without_token(&query),
        body: body.to_vec(),
        sender_id: sender_id_of(&headers),
        cause: model::Cause {
            actor: orch::door_by(),
            ..model::Cause::default()
        },
    };
    match door.to.emit(emission).await {
        Ok(got) => {
            let id = serde_json::Value::String(got.id());
            (
                StatusCode::ACCEPTED,
                [(CONTENT_TYPE, "application/json")],
                format!(r#"{{"event":{id}}}"#),
            )
                .into_response()
        }
        // gone or paused since the key was checked
        Err(orch::Error::NoEventSignal | orch::Error::SignalPaused) => refuse(StatusCode::UNAUTHORIZED, "unauthorized"),
        Err(err) => {
            tracing::error!(signal = %es.name, %err, "event not stored");
            refuse(StatusCode::INTERNAL_SERVER_ERROR, "the event could not be stored")
        }
    }
}

/// An answer that refuses, in plain text.
fn refuse(status: StatusCode, why: &str) -> Response {
    (
        status,
        [
            (CONTENT_TYPE, "text/plain; charset=utf-8"),
            (X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        format!("{why}\n"),
    )
        .into_response()
}

/// The first value of header name, as text; none when it is missing or is not text.
fn first<'a>(h: &'a HeaderMap, name: &str) -> &'a str {
    h.get(name).and_then(|v| v.to_str().ok()).unwrap_or_default()
}

/// Who wrote a post: the CLI when it says so, the door else.
fn writer_of(h: &HeaderMap) -> Writer {
    let kind = if first(h, VIA_HEADER) == EventVia::Cli.as_str() {
        WriterKind::Cli
    } else {
        WriterKind::Door
    };
    Writer {
        kind,
        ..Writer::default()
    }
}

/// The pairs of uri's query, decoded, in their order.
fn query_of(uri: &Uri) -> Vec<(String, String)> {
    form_urlencoded::parse(uri.query().unwrap_or_default().as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

/// What a post shows of the key: the key it carries, and the signatures of the senders that sign, each over the message that
/// sender signs (GitHub: the body; Stripe: its stamp, a dot and the body; Slack: v0, its stamp and the body, with colons). A
/// signature whose stamp is not within `STAMP_TOLERANCE_SECS` of now is no proof: it is a replay.
fn proof_of(h: &HeaderMap, query: &[(String, String)], body: &[u8], now: Time) -> KeyProof {
    let mut p = KeyProof {
        key: presented_key(h, query),
        signed: Vec::new(),
    };
    if let Some(hex_mac) = first(h, "X-Hub-Signature-256").strip_prefix("sha256=") {
        p.signed.push(signed_by(body.to_vec(), hex_mac));
    }
    let (stamp, macs) = stripe_signature(first(h, "Stripe-Signature"));
    if fresh(stamp, now) {
        for m in macs {
            p.signed.push(signed_by(joined(&[stamp, "."], body), m));
        }
    }
    let stamp = first(h, "X-Slack-Request-Timestamp");
    if fresh(stamp, now)
        && let Some(hex_mac) = first(h, "X-Slack-Signature").strip_prefix("v0=")
    {
        p.signed.push(signed_by(joined(&["v0:", stamp, ":"], body), hex_mac));
    }
    let hex_mac = first(h, "X-Grafana-Alerting-Signature");
    if !hex_mac.is_empty() {
        // Grafana names no stamp header of its own: the owner sets this one as the contact point's Timestamp Header. With it
        // the signature is of "<stamp>:<body>" and must be fresh; without it, of the body alone, as GitHub's.
        let stamps: Vec<_> = h.get_all("X-Grafana-Alerting-Signature-Timestamp").iter().collect();
        match stamps.as_slice() {
            [] => p.signed.push(signed_by(body.to_vec(), hex_mac)),
            [stamp] => {
                let stamp = stamp.to_str().unwrap_or_default();
                if fresh(stamp, now) {
                    p.signed.push(signed_by(joined(&[stamp, ":"], body), hex_mac));
                }
            }
            _ => {}
        }
    }
    p
}

/// The parts, then body, as one message.
fn joined(parts: &[&str], body: &[u8]) -> Vec<u8> {
    let mut m: Vec<u8> = parts.concat().into_bytes();
    m.extend_from_slice(body);
    m
}

/// message with the MAC a sender gave in hex; a MAC that is not hex signs nothing, and stays empty.
fn signed_by(message: Vec<u8>, hex_mac: &str) -> Signed {
    Signed {
        message,
        mac: hex::decode(hex_mac).unwrap_or_default(),
    }
}

/// The stamp and the v1 signatures of a Stripe-Signature header, "t=<unix>,v1=<hex>[,v1=<hex>]".
fn stripe_signature(header: &str) -> (&str, Vec<&str>) {
    let mut stamp = "";
    let mut macs = Vec::new();
    for part in header.split(',') {
        let (k, v) = part.trim().split_once('=').unwrap_or((part.trim(), ""));
        match k {
            "t" => stamp = v,
            "v1" => macs.push(v),
            _ => {}
        }
    }
    (stamp, macs)
}

/// Whether a stamp (unix seconds, as a sender writes it) is within `STAMP_TOLERANCE_SECS` of now.
fn fresh(stamp: &str, now: Time) -> bool {
    let Ok(sec) = stamp.parse::<i64>() else {
        return false;
    };
    let tolerance = STAMP_TOLERANCE_SECS.saturating_mul(1_000_000_000);
    i128::from(sec)
        .checked_mul(1_000_000_000)
        .and_then(|at| now.0.unix_timestamp_nanos().checked_sub(at))
        .is_some_and(|d| d.unsigned_abs() <= tolerance.unsigned_abs())
}

/// The challenge of a post that only asks the address to prove itself (`{"type":"url_verification","challenge":"…"}`, as
/// Slack asks): it is answered and kept as no event. It is read after the post was let in, so only the owner's signal answers
/// one.
fn challenge_in(body: &[u8]) -> Option<String> {
    let c: serde_json::Value = serde_json::from_slice(body).ok()?;
    if c.get("type")?.as_str()? != "url_verification" {
        return None;
    }
    let challenge = c.get("challenge")?.as_str()?;
    (!challenge.is_empty()).then(|| challenge.to_owned())
}

/// The key a post carries, in the first place the sender put one: GitLab's token header, a bearer token, or the query for a
/// sender that can set neither.
fn presented_key(h: &HeaderMap, query: &[(String, String)]) -> String {
    let authorization = first(h, "Authorization");
    let token = query
        .iter()
        .find(|(k, _)| k == "token")
        .map(|(_, v)| v.as_str())
        .unwrap_or_default();
    [
        first(h, "X-Gitlab-Token"),
        authorization.strip_prefix("Bearer ").unwrap_or(authorization),
        token,
    ]
    .into_iter()
    .find(|k| !k.is_empty())
    .unwrap_or_default()
    .to_owned()
}

/// The sender's own number for the post, from the first header that gives one.
fn sender_id_of(h: &HeaderMap) -> String {
    SENDER_ID_HEADERS
        .iter()
        .map(|k| first(h, k).trim())
        .find(|v| !v.is_empty())
        .unwrap_or_default()
        .to_owned()
}

/// Every header the sender set but those that may carry a credential, one value each, keyed by its canonical name, each
/// word capitalised (`X-Gitlab-Event`), which is how a subscription's `when` reads it.
fn headers_of(h: &HeaderMap) -> BTreeMap<String, String> {
    h.keys()
        .filter(|k| {
            let k = k.as_str();
            !TRANSPORT_HEADERS.contains(&k) && !CREDENTIAL_WORDS.iter().any(|w| k.contains(w))
        })
        .map(|k| {
            let values: Vec<_> = h
                .get_all(k)
                .iter()
                .map(|v| String::from_utf8_lossy(v.as_bytes()).into_owned())
                .collect();
            (canonical(k.as_str()), values.join(", "))
        })
        .collect()
}

/// A header's name (lowercase, as the HTTP layer gives it) with the first letter and each after a hyphen in upper case.
fn canonical(name: &str) -> String {
    let mut upper = true;
    name.chars()
        .map(|c| {
            let out = if upper { c.to_ascii_uppercase() } else { c };
            upper = c == '-';
            out
        })
        .collect()
}

/// The query with the key's parameter taken out, its names in order.
fn without_token(query: &[(String, String)]) -> String {
    let mut by_name: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (k, v) in query.iter().filter(|(k, _)| k != "token") {
        by_name.entry(k).or_default().push(v);
    }
    let mut out = form_urlencoded::Serializer::new(String::new());
    for (k, values) in by_name {
        for v in values {
            out.append_pair(k, v);
        }
    }
    out.finish()
}
