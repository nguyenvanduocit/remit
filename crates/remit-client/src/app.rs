//! `remit-client call|snapshot|watch`: the owner's contract (`wire::app`) from a shell, so a person or an agent can call
//! the server and watch what it pushes with no browser in between (ADR-0072 decision 10). A session presents the owner
//! key and names no web bundle; each command opens one, does its one thing and ends. The calls are the contract's own,
//! typed by `call::outgoing`: nothing here is written per call.

use link::{ErrorCode, Event, Failure, Hello, Node, Request};
use link_quic::Peer;
use serde_json::Value;
use tokio::io::{AsyncWrite, AsyncWriteExt as _};
use wire::app;

use crate::call::{Salt, outgoing};
use crate::dial::{self, Down};

/// The server to reach (`http://host:port`, where its `/link.json` is) and the owner key to come in with.
#[derive(Debug, Clone)]
pub struct Owner {
    pub server: String,
    pub key: String,
}

/// A session on the server, admitted to `wire::app` with the owner key.
async fn session(o: &Owner) -> Result<Peer, Failure> {
    let hello = Hello {
        node: Node {
            id: "remit-client".to_owned(),
            public_key: String::new(),
        },
        credential: o.key.clone(),
        capabilities: vec![app::CAPABILITY.to_owned()],
        offers: Vec::new(),
        bundle: None,
    };
    dial::open(&o.server, hello).await.map_err(|down| match down {
        Down::Refused(r) => Failure::new(
            ErrorCode::Forbidden,
            format!("the server refused the session: {}", r.reason),
        ),
        Down::Unreachable(why) => Failure::new(ErrorCode::Disconnected, why),
    })
}

/// Calls `method` with `args` (a changing call with `request`, else a new one) and returns its answer. A call that is
/// not the contract's, or whose args do not decode, is refused before any session opens.
pub async fn call(o: &Owner, method: &str, args: Value, request: Option<Request>) -> Result<Value, Failure> {
    let out = outgoing::<app::Call>(app::CALLS, method, args, request, &mut Salt::new())?;
    let answer = session(o).await?.link.call_encoded(out).await?;
    serde_json::to_value(app::decode_answer(method, &answer)?)
        .map_err(|e| Failure::new(ErrorCode::InternalError, format!("the answer as JSON: {e}")))
}

/// The snapshot a channel starts from.
pub async fn snapshot(o: &Owner) -> Result<Value, Failure> {
    call(o, "app_snapshot", Value::Null, None).await
}

/// Writes each push of `wire::app` to `out` as one JSON line, `{"push": <name>, "args": {...}}`, until the session
/// ends; why it ended.
pub async fn watch(o: &Owner, mut out: impl AsyncWrite + Unpin) -> Failure {
    let mut peer = match session(o).await {
        Ok(peer) => peer,
        Err(failure) => return failure,
    };
    loop {
        match peer.events.recv().await {
            Some(Event::Push { contract, name, body }) if contract == app::CAPABILITY => {
                let push = match link::decode_push::<app::Push>(&name, &body).and_then(|push| {
                    serde_json::to_string(&push).map_err(|e| Failure::new(ErrorCode::InternalError, e.to_string()))
                }) {
                    Ok(push) => push,
                    Err(failure) => return failure,
                };
                let mut line = push;
                line.push('\n');
                if let Err(e) = out.write_all(line.as_bytes()).await.and(out.flush().await) {
                    return Failure::new(ErrorCode::Aborted, format!("write a push: {e}"));
                }
            }
            Some(Event::Call { id, .. }) => peer.link.reply(
                id,
                Err(Failure::new(ErrorCode::Forbidden, "a shell client answers no calls")),
            ),
            Some(Event::Closed(closing)) => return Failure::new(ErrorCode::Disconnected, dial::why_closed(&closing)),
            None => return Failure::new(ErrorCode::Disconnected, "the session ended"),
            Some(_) => {}
        }
    }
}
