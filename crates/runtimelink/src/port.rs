//! What the core and the runtime need of one link session, declared here by its consumers: the shape of
//! `link_quic::Link`, which the binary wires in, and of `memory::Link`, which the tests use.

use std::future::Future;

use link::{Contract, ErrorCode, Event, Failure, Incoming, Outgoing, Payload, Refusal};
use serde::de::DeserializeOwned;
use tokio::sync::mpsc;

/// A handle on one session of the link; cloning it is another handle on the same session.
pub trait Link: Clone + Send + Sync + 'static {
    /// Calls the peer and waits for the answer, or for why there is none (`TIMEOUT`, `DISCONNECTED`, the peer's code).
    fn call<C, A>(&self, out: Outgoing<C, A>) -> impl Future<Output = Result<A, Failure>> + Send
    where
        C: Contract + Send + 'static,
        A: DeserializeOwned + Send + 'static;
    /// Answers a call the peer made (`Event::Call`).
    fn reply(&self, id: Incoming, result: Result<Payload, Failure>);
    /// Lets the dialler in after its `Event::Hello`, granting it these contracts.
    fn admit(&self, granted: Vec<String>);
    /// Turns the dialler away, and closes the session.
    fn refuse(&self, refusal: Refusal);
    fn close(&self);
}

/// One session: `link` to act on it, `events` for what the peer does (its hello, its calls, the session's end). Answers
/// to this side's calls are not among the events: `Link::call` returns them.
pub struct Peer<L> {
    pub link: L,
    pub events: mpsc::UnboundedReceiver<Event>,
}

/// Says a call was lost with the session it went out on: whether the peer acted is not known, and the same call sent on
/// the next session does what it would have done (every call of `wire::runtime` acts once by the id it carries).
pub fn dropped(failure: &Failure) -> bool {
    failure.code == ErrorCode::Disconnected
}

/// One session of a peer, told apart from the sessions before and after it.
#[derive(Clone)]
pub(crate) struct Connection<L> {
    pub(crate) n: u64,
    pub(crate) link: L,
}
