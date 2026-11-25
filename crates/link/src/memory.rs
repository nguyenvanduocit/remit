//! Two halves wired back to back in memory, on a clock the caller moves: the transport of the protocol's tests, of the
//! client core's tests and of a fake shell (`web/verify`). It can hold what the dialler sends, to deliver it in another
//! order, and stop the dialler reading, to make it fall behind.

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use crate::conn::{Closing, Config, Conn, Event, Millis, StreamId, Transport};
use crate::envelope::{Hello, Node};

#[derive(Debug)]
enum Op {
    Send(StreamId, Vec<u8>),
    Close(Closing),
}

#[derive(Debug, Default)]
struct Wire {
    ops: Vec<Op>,
}

impl Transport for Wire {
    fn open(&mut self, _: StreamId) {}

    fn send(&mut self, stream: StreamId, bytes: Vec<u8>) {
        self.ops.push(Op::Send(stream, bytes));
    }

    fn finish(&mut self, _: StreamId) {}

    fn reset(&mut self, _: StreamId) {}

    fn close(&mut self, closing: Closing) {
        self.ops.push(Op::Close(closing));
    }
}

#[derive(Debug)]
pub struct Pair {
    pub now: Millis,
    pub dialer: Conn,
    pub acceptor: Conn,
    pub dialer_events: Vec<Event>,
    pub acceptor_events: Vec<Event>,
    /// While false, what the acceptor sends stays unread and unflushed.
    pub dialer_reads: bool,
    /// While true, what the dialler sends is held in `held` instead of delivered.
    pub hold_to_acceptor: bool,
    pub held: Vec<(StreamId, Vec<u8>)>,
}

impl Pair {
    pub fn new(dialer: Conn, acceptor: Conn) -> Self {
        let mut pair = Self {
            now: 0,
            dialer,
            acceptor,
            dialer_events: Vec::new(),
            acceptor_events: Vec::new(),
            dialer_reads: true,
            hold_to_acceptor: false,
            held: Vec::new(),
        };
        pair.settle();
        pair
    }

    /// A session already admitted: the dialler asks for `capabilities` and is granted them, and offers `offers`.
    pub fn admitted(cfg: Config, capabilities: &[&str], offers: &[&str]) -> Self {
        let owned = |list: &[&str]| list.iter().map(|c| (*c).to_owned()).collect::<Vec<_>>();
        let hello = Hello {
            node: node("dialer"),
            credential: "key".to_owned(),
            capabilities: owned(capabilities),
            offers: owned(offers),
            bundle: None,
        };
        let mut pair = Self::new(Conn::dial(cfg, hello), Conn::accept(cfg, node("acceptor")));
        pair.acceptor.admit(owned(capabilities));
        pair.settle();
        pair.dialer_events.clear();
        pair.acceptor_events.clear();
        pair
    }

    /// Moves everything either half sent until neither has more to send.
    pub fn settle(&mut self) {
        loop {
            let mut from_dialer = Wire::default();
            let mut from_acceptor = Wire::default();
            self.dialer_events.extend(self.dialer.drive(&mut from_dialer));
            self.acceptor_events.extend(self.acceptor.drive(&mut from_acceptor));
            if from_dialer.ops.is_empty() && from_acceptor.ops.is_empty() {
                return;
            }
            for op in from_dialer.ops {
                match op {
                    Op::Send(stream, bytes) if self.hold_to_acceptor => self.held.push((stream, bytes)),
                    Op::Send(stream, bytes) => {
                        self.acceptor.receive(stream, &bytes);
                        self.dialer.written(stream, bytes.len());
                    }
                    Op::Close(closing) => self.acceptor.closed(closing),
                }
            }
            for op in from_acceptor.ops {
                match op {
                    Op::Send(stream, bytes) => {
                        if self.dialer_reads {
                            self.dialer.receive(stream, &bytes);
                            self.acceptor.written(stream, bytes.len());
                        }
                    }
                    Op::Close(closing) => self.dialer.closed(closing),
                }
            }
        }
    }

    /// Delivers what was held, the last sent first.
    pub fn release_reversed(&mut self) {
        self.hold_to_acceptor = false;
        for (stream, bytes) in std::mem::take(&mut self.held).into_iter().rev() {
            self.acceptor.receive(stream, &bytes);
            self.dialer.written(stream, bytes.len());
        }
        self.settle();
    }

    /// Moves the clock and lets both halves act on it.
    pub fn advance(&mut self, by: Millis) {
        self.now = self.now.saturating_add(by);
        self.dialer.tick(self.now);
        self.acceptor.tick(self.now);
        self.settle();
    }
}

pub fn node(id: &str) -> Node {
    Node {
        id: id.to_owned(),
        public_key: String::new(),
    }
}

/// Runs a future that is ready at once (a handler over memory); none when it would wait.
pub fn now_or_never<F: Future>(future: F) -> Option<F::Output> {
    match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(out) => Some(out),
        Poll::Pending => None,
    }
}
