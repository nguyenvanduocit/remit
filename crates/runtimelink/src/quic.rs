//! A session of the link over QUIC (`link_quic`), as the core and the runtime use one: the transport the binary wires in.

use link::{Contract, Failure, Incoming, Outgoing, Payload, Refusal};
use serde::de::DeserializeOwned;

use crate::port::{self, Peer};

impl port::Link for link_quic::Link {
    async fn call<C, A>(&self, out: Outgoing<C, A>) -> Result<A, Failure>
    where
        C: Contract + Send + 'static,
        A: DeserializeOwned + Send + 'static,
    {
        link_quic::Link::call(self, out).await
    }

    fn reply(&self, id: Incoming, result: Result<Payload, Failure>) {
        link_quic::Link::reply(self, id, result);
    }

    fn admit(&self, granted: Vec<String>) {
        link_quic::Link::admit(self, granted);
    }

    fn refuse(&self, refusal: Refusal) {
        link_quic::Link::refuse(self, refusal);
    }

    fn close(&self) {
        link_quic::Link::close(self);
    }
}

impl From<link_quic::Peer> for Peer<link_quic::Link> {
    fn from(p: link_quic::Peer) -> Self {
        Peer {
            link: p.link,
            events: p.events,
        }
    }
}
