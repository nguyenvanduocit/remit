//! A session's streams, with no `Conn`: what a conn asks of its `Transport` is carried out on the session (a task per
//! stream), and what the session does comes back from `next` as the conn takes it, the peer's streams numbered as the
//! conn knows them. The driver owns one beside its own `Conn`; a shell whose conn lives elsewhere (the client core,
//! ADR-0128) owns one alone, as `client-web`'s browser shell does over WebTransport.

use std::collections::HashMap;

use bytes::Bytes;
use link::{Closing, Side, StreamId, StreamKind, Transport};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use web_transport_quinn::{RecvStream, SendStream, Session, SessionError, WebTransportError};

use crate::stream::{self, Write};

/// What the session's own tasks tell its `Streams`.
pub(crate) enum Input {
    Read(StreamId, Bytes),
    Written(StreamId, usize),
    AcceptedBi(SendStream, RecvStream),
    AcceptedUni(RecvStream),
    /// A stream's task ended: neither side has more to say on it.
    Ended(StreamId),
    Closed(Closing),
}

/// What happened on the session that its conn takes: `Conn::receive`, `Conn::written`, `Conn::closed`.
#[derive(Debug, Clone, PartialEq)]
pub enum Happened {
    Read(StreamId, Bytes),
    Written(StreamId, usize),
    Closed(Closing),
}

pub struct Streams {
    session: Session,
    /// The other side: the opener of the streams it opens.
    peer: Side,
    inputs: mpsc::UnboundedSender<Input>,
    arrived: mpsc::UnboundedReceiver<Input>,
    writers: HashMap<StreamId, mpsc::UnboundedSender<Write>>,
    tasks: Vec<JoinHandle<()>>,
    peer_bi: u64,
    peer_uni: u64,
}

impl Streams {
    /// The streams of an open `session` whose other side is `peer`; must run inside a tokio runtime.
    #[must_use]
    pub fn start(session: Session, peer: Side) -> Self {
        let (inputs, arrived) = mpsc::unbounded_channel();
        let tasks = vec![
            tokio::spawn(accept_bi(session.clone(), inputs.clone())),
            tokio::spawn(accept_uni(session.clone(), inputs.clone())),
            tokio::spawn(watch_closed(session.clone(), inputs.clone())),
        ];
        Self {
            session,
            peer,
            inputs,
            arrived,
            writers: HashMap::new(),
            tasks,
            peer_bi: 0,
            peer_uni: 0,
        }
    }

    /// Where the other side's packets come from, as the QUIC connection sees it.
    #[must_use]
    pub fn remote_address(&self) -> std::net::SocketAddr {
        self.session.remote_address()
    }

    /// The next thing that happened on the session for the conn. Cancel-safe: a stream the peer opened is taken in
    /// before it waits again.
    pub async fn next(&mut self) -> Happened {
        loop {
            // `self` holds a sender, so the channel never closes.
            let Some(input) = self.arrived.recv().await else {
                return Happened::Closed(Closing::Done);
            };
            match input {
                Input::Read(id, bytes) => return Happened::Read(id, bytes),
                Input::Written(id, n) => return Happened::Written(id, n),
                Input::Closed(closing) => return Happened::Closed(closing),
                Input::AcceptedBi(send, recv) => {
                    let id = self.peer_stream(StreamKind::Bi);
                    self.spawn(id, stream::Ends::Both(send, recv));
                }
                Input::AcceptedUni(recv) => {
                    let id = self.peer_stream(StreamKind::Uni);
                    self.spawn(id, stream::Ends::Recv(recv));
                }
                Input::Ended(id) => {
                    self.writers.remove(&id);
                }
            }
        }
    }

    /// The peer's streams are numbered here in the order they are accepted, per kind: the conn needs only that its
    /// ids are distinct and say who opened the stream and how.
    fn peer_stream(&mut self, kind: StreamKind) -> StreamId {
        let counter = match kind {
            StreamKind::Bi => &mut self.peer_bi,
            StreamKind::Uni => &mut self.peer_uni,
        };
        let n = *counter;
        *counter = counter.wrapping_add(1);
        StreamId {
            opener: self.peer,
            kind,
            n,
        }
    }

    fn spawn(&mut self, id: StreamId, ends: stream::Ends) {
        let (tx, rx) = mpsc::unbounded_channel();
        self.writers.insert(id, tx);
        self.tasks
            .push(tokio::spawn(stream::run(id, ends, rx, self.inputs.clone())));
    }

    fn write(&self, id: StreamId, write: Write) {
        if let Some(writer) = self.writers.get(&id) {
            // A stream whose task ended (the peer reset it) takes nothing more.
            writer.send(write).ok();
        }
    }
}

impl Transport for Streams {
    fn open(&mut self, stream: StreamId) {
        let ends = match stream.kind {
            StreamKind::Bi => stream::Ends::OpenBi(self.session.clone()),
            StreamKind::Uni => stream::Ends::OpenUni(self.session.clone()),
        };
        self.spawn(stream, ends);
    }

    fn send(&mut self, stream: StreamId, bytes: Vec<u8>) {
        self.write(stream, Write::Bytes(bytes));
    }

    fn finish(&mut self, stream: StreamId) {
        self.write(stream, Write::Finish);
    }

    fn reset(&mut self, stream: StreamId) {
        self.write(stream, Write::Reset);
    }

    fn close(&mut self, closing: Closing) {
        let (code, reason) = closing.code();
        self.session.close(code, reason.as_bytes());
    }
}

impl Drop for Streams {
    /// A session let go ends its stream tasks.
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

async fn accept_bi(session: Session, inputs: mpsc::UnboundedSender<Input>) {
    while let Ok((send, recv)) = session.accept_bi().await {
        if inputs.send(Input::AcceptedBi(send, recv)).is_err() {
            return;
        }
    }
}

async fn accept_uni(session: Session, inputs: mpsc::UnboundedSender<Input>) {
    while let Ok(recv) = session.accept_uni().await {
        if inputs.send(Input::AcceptedUni(recv)).is_err() {
            return;
        }
    }
}

async fn watch_closed(session: Session, inputs: mpsc::UnboundedSender<Input>) {
    let closing = match session.closed().await {
        SessionError::WebTransportError(WebTransportError::Closed(code, reason)) => Closing::from_code(code, &reason),
        _ => Closing::Done,
    };
    inputs.send(Input::Closed(closing)).ok();
}
