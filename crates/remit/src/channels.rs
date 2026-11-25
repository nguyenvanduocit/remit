//! The link's door (ADR-0072 decisions 3 and 12): one WebTransport listener at `/link` for every peer, each session handed,
//! by the contract its hello asks for, to the channel that serves it: `wire::app` to the owner's channels (`rpc`),
//! `wire::runtime`'s `to_core` to the AI runtime's link (`runtimelink`). Each channel
//! checks the hello's credential itself and refuses or admits.

use link::{Event, Hello, Refusal, RefusalCode};
use link_quic::{Listener, Peer};
use tokio_util::sync::CancellationToken;
use wire::runtime::to_core;

use crate::ports::RuntimeLink;

/// The channels a session on the link can be served by.
#[derive(Clone)]
pub struct Channels {
    pub app: rpc::Server,
    pub runtime: runtimelink::Launcher<RuntimeLink>,
    /// Cancelled when the server begins to stop: the browsers leave first, and none comes in after.
    pub browsers: CancellationToken,
}

/// The channel a hello is for, by the first contract it asks to call that a channel here serves.
#[derive(Debug, PartialEq, Eq)]
enum For {
    App,
    Runtime,
    Nothing,
}

fn channel_of(hello: &Hello) -> For {
    for c in &hello.capabilities {
        match c.as_str() {
            wire::app::CAPABILITY => return For::App,
            to_core::CAPABILITY => return For::Runtime,
            _ => {}
        }
    }
    For::Nothing
}

impl Channels {
    /// Accepts sessions until `stop` is cancelled, each served on a task of its own.
    pub async fn serve(self, mut listener: Listener, stop: CancellationToken) {
        loop {
            let peer = tokio::select! {
                () = stop.cancelled() => return,
                next = listener.accept() => match next {
                    None => return,
                    Some(Ok(peer)) => peer,
                    Some(Err(err)) => {
                        tracing::warn!(%err, "a session on the link did not open");
                        continue;
                    }
                },
            };
            tokio::spawn(self.clone().session(peer));
        }
    }

    /// Serves one session: its first event is its hello.
    async fn session(self, mut peer: Peer) {
        let Some(Event::Hello(hello)) = peer.events.recv().await else {
            return;
        };
        match channel_of(&hello) {
            For::App => {
                let link = peer.link.clone();
                // An action a browser asked for runs to its end (orch's `act`); only the session goes.
                tokio::select! {
                    () = self.browsers.cancelled() => link.close(),
                    () = self.app.serve(hello, peer) => {}
                }
            }
            For::Runtime => self.runtime.serve(hello, peer.into()).await,
            For::Nothing => refuse(&peer, "no contract asked for is served here"),
        }
    }
}

fn refuse(peer: &Peer, why: &str) {
    peer.link.refuse(Refusal {
        code: RefusalCode::BadCredential,
        reason: why.to_owned(),
    });
}
