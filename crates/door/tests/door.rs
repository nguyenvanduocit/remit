//! The door as its mount serves it: a post is let in only with the key of the signal it names, in any place a sender puts
//! one or signed the way that sender signs, and every refusal reads the same.
#![expect(
    clippy::unwrap_used,
    reason = "test helpers outside #[test] fns, where unwrap is the assertion"
)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use door::{Emitter, Keys, router};
use hmac::{Hmac, Mac};
use model::{EventSignal, Reading, Time, event_key};
use orch::{Emission, Emitted, KeyProof};
use sha2::Sha256;
use tower::ServiceExt;

/// Remits a post as Remit does, keeping what it was handed.
#[derive(Default)]
struct FakeEmitter {
    got: Mutex<Vec<Emission>>,
}

impl Emitter for FakeEmitter {
    async fn emit(&self, e: Emission) -> orch::Result<Emitted> {
        self.got.lock().unwrap().push(e);
        Ok(Emitted {
            reading: Reading {
                key: event_key("gitlab"),
                rev: 1,
                ..Reading::default()
            },
            ..Emitted::default()
        })
    }
}

/// Opens the door of each event signal with its own key, as Remit does.
#[derive(Default, Clone)]
struct FakeKeys {
    /// by name
    keys: BTreeMap<String, String>,
    paused: Vec<String>,
}

impl Keys for FakeKeys {
    async fn authorize_event_signal(&self, name: &str, proof: &KeyProof) -> orch::Result<Option<EventSignal>> {
        let name = name.to_lowercase();
        let Some(want) = self.keys.get(&name) else {
            return Ok(None);
        };
        if self.paused.contains(&name) || !proof.opens(want) {
            return Ok(None);
        }
        Ok(Some(EventSignal {
            id: format!("es_{name}"),
            name,
            ..EventSignal::default()
        }))
    }
}

fn keys(pairs: &[(&str, &str)]) -> FakeKeys {
    FakeKeys {
        keys: pairs.iter().map(|(n, k)| ((*n).to_owned(), (*k).to_owned())).collect(),
        paused: Vec::new(),
    }
}

fn two_signals() -> FakeKeys {
    keys(&[("gitlab", "whk_gitlab-key"), ("sentry", "whk_sentry-key")])
}

fn door_at(keys: FakeKeys, now: Time) -> (Router, Arc<FakeEmitter>) {
    let to = Arc::new(FakeEmitter::default());
    (router(Arc::new(keys), to.clone(), Arc::new(move || now)), to)
}

fn now() -> Time {
    Time(time::OffsetDateTime::now_utc())
}

/// The status and body of a request with method to uri.
async fn send(door: &Router, method: Method, uri: &str, body: &str, header: &[(&str, &str)]) -> (StatusCode, String) {
    let mut req = Request::builder().method(method).uri(uri);
    for (k, v) in header {
        req = req.header(*k, *v);
    }
    let res = door
        .clone()
        .oneshot(req.body(Body::from(body.to_owned())).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let answer = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8(answer.to_vec()).unwrap())
}

async fn post(door: &Router, uri: &str, body: &str, header: &[(&str, &str)]) -> (StatusCode, String) {
    send(door, Method::POST, uri, body, header).await
}

/// A case of a post: what it is, where it goes, its headers and the status it gets.
type Case<'a> = (&'a str, &'a str, &'a [(&'a str, &'a str)], StatusCode);

/// hex of the HMAC-SHA256 of message under key: GitHub's signature of a body.
fn mac(key: &str, message: &str) -> String {
    let mut m = Hmac::<Sha256>::new_from_slice(key.as_bytes()).unwrap();
    m.update(message.as_bytes());
    hex::encode(m.finalize().into_bytes())
}

// An event is let in through the door of the event signal it names, with that signal's own key, in any of the places a
// sender can put one.
#[tokio::test]
async fn a_post_is_let_in_only_with_the_key_of_the_signal_it_names() {
    let (door, r) = door_at(two_signals(), now());
    let github = format!("sha256={}", mac("whk_gitlab-key", "{}"));
    let other_key = format!("sha256={}", mac("whk_sentry-key", "{}"));
    let other_body = format!("sha256={}", mac("whk_gitlab-key", "[]"));
    let cases: [Case<'_>; 13] = [
        ("no key", "/hooks/gitlab", &[], StatusCode::UNAUTHORIZED),
        ("a wrong key", "/hooks/gitlab?token=nope", &[], StatusCode::UNAUTHORIZED),
        (
            "another signal's key",
            "/hooks/gitlab",
            &[("X-Gitlab-Token", "whk_sentry-key")],
            StatusCode::UNAUTHORIZED,
        ),
        (
            "a signal nobody made",
            "/hooks/jira",
            &[("X-Gitlab-Token", "whk_gitlab-key")],
            StatusCode::UNAUTHORIZED,
        ),
        (
            "the key with something more",
            "/hooks/gitlab",
            &[("X-Gitlab-Token", "whk_gitlab-keyx")],
            StatusCode::UNAUTHORIZED,
        ),
        (
            "GitLab's header",
            "/hooks/gitlab",
            &[("X-Gitlab-Token", "whk_gitlab-key")],
            StatusCode::ACCEPTED,
        ),
        (
            "a bearer token",
            "/hooks/sentry",
            &[("Authorization", "Bearer whk_sentry-key")],
            StatusCode::ACCEPTED,
        ),
        (
            "the query",
            "/hooks/sentry?token=whk_sentry-key",
            &[],
            StatusCode::ACCEPTED,
        ),
        (
            "the name in other letters",
            "/hooks/GitLab",
            &[("X-Gitlab-Token", "whk_gitlab-key")],
            StatusCode::ACCEPTED,
        ),
        (
            "GitHub's signature",
            "/hooks/gitlab",
            &[("X-Hub-Signature-256", &github)],
            StatusCode::ACCEPTED,
        ),
        (
            "a signature with another key",
            "/hooks/gitlab",
            &[("X-Hub-Signature-256", &other_key)],
            StatusCode::UNAUTHORIZED,
        ),
        (
            "a signature of another body",
            "/hooks/gitlab",
            &[("X-Hub-Signature-256", &other_body)],
            StatusCode::UNAUTHORIZED,
        ),
        (
            "a signature not in hex",
            "/hooks/gitlab",
            &[("X-Hub-Signature-256", "sha256=whk_gitlab-key")],
            StatusCode::UNAUTHORIZED,
        ),
    ];
    for (name, uri, header, want) in cases {
        let (status, _) = post(&door, uri, "{}", header).await;
        assert_eq!(status, want, "{name}");
    }
    let got = r.got.lock().unwrap();
    assert_eq!(got.len(), 5, "only the 5 with the right key reach Remit");
    for e in got.iter() {
        assert!(
            e.signal == "gitlab" || e.signal == "sentry",
            "an event carries the signal's name, not what the URL said: {:?}",
            e.signal
        );
    }
}
