//! A `link` session over one byte pipe each way (a child's stdio): the driver of a `link::Conn` that the plugin's `serve` and
//! Remit's `RemotePlugin` share. The pipe carries records, `len (u32 BE) ‖ body`, a body being one of
//!
//! - `0 ‖ opener (0 dialer, 1 acceptor) ‖ kind (0 bi, 1 uni) ‖ n (u64 BE) ‖ bytes`: what a stream carries;
//! - `1 ‖ 0`, `1 ‖ 1 ‖ <JSON refusal>`, `1 ‖ 2`, `1 ‖ 3 ‖ <UTF-8 why>`: the session closes (done, refused, behind, broken).
//!
//! A stream is named by its `link` id on both ends, so the peer learns of it from its first bytes; opening, finishing and
//! resetting a stream say nothing to the peer's `Conn`, so they are not sent. The end of the pipe is the end of the session.
//! Three tasks run a session: one reads records, one writes them (so a peer that is not reading never stops this one from
//! reading), and one owns the `Conn`, which every `Peer` handle reaches through a queue of commands.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use link::{
    CallId, Closing, Conn, Contract, ErrorCode, Event, Failure, Hello, Millis, Outgoing, Payload, Pushes, Refusal,
    Side, StreamId, StreamKind, Transport, Welcome,
};
use serde::de::DeserializeOwned;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

/// The largest record read: a frame of a frontend's asset fits; a length past it is a broken pipe, not an allocation.
pub const MAX_RECORD: usize = 64 << 20;

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// How this end answers the calls the peer makes: by contract and method, the call's arguments in, its answer out.
pub type Serve = Arc<dyn Fn(String, String, Payload) -> BoxFuture<Result<Payload, Failure>> + Send + Sync>;

/// What the session tells the end that runs it, besides the answers to its calls.
#[derive(Debug, Clone, PartialEq)]
pub enum Note {
    /// The acceptor's: the dialler said hello; `Peer::admit` or `Peer::refuse` it.
    Hello(Hello),
    /// The dialler's: it was let in.
    Welcomed(Welcome),
    Refused(Refusal),
    Push {
        contract: String,
        name: String,
        body: Payload,
    },
    /// The last note.
    Closed(Closing),
}

/// One record of the pipe.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Record {
    Send(StreamId, Vec<u8>),
    Close(Closing),
}

/// The record's bytes, its length first.
pub(crate) fn encode(record: &Record) -> Vec<u8> {
    let mut body = Vec::new();
    match record {
        Record::Send(stream, bytes) => {
            body.push(0);
            body.push(match stream.opener {
                Side::Dialer => 0,
                Side::Acceptor => 1,
            });
            body.push(match stream.kind {
                StreamKind::Bi => 0,
                StreamKind::Uni => 1,
            });
            body.extend_from_slice(&stream.n.to_be_bytes());
            body.extend_from_slice(bytes);
        }
        Record::Close(closing) => {
            body.push(1);
            match closing {
                Closing::Done => body.push(0),
                Closing::Refused(refusal) => {
                    body.push(1);
                    body.extend(serde_json::to_vec(refusal).unwrap_or_default());
                }
                Closing::Behind => body.push(2),
                Closing::Broken(why) => {
                    body.push(3);
                    body.extend_from_slice(why.as_bytes());
                }
            }
        }
    }
    // A body over 4 GiB is cut to what its length says; `MAX_RECORD` refuses it on the other end long before.
    let len = u32::try_from(body.len()).unwrap_or(u32::MAX);
    let mut out = Vec::with_capacity(body.len().saturating_add(4));
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&body);
    out
}

/// The record a body (what follows its length) is.
pub(crate) fn decode(body: &[u8]) -> Result<Record, String> {
    match body {
        [0, opener, kind, rest @ ..] => {
            let opener = match opener {
                0 => Side::Dialer,
                1 => Side::Acceptor,
                other => return Err(format!("no stream opener {other}")),
            };
            let kind = match kind {
                0 => StreamKind::Bi,
                1 => StreamKind::Uni,
                other => return Err(format!("no stream kind {other}")),
            };
            let (n, bytes) = rest.split_first_chunk::<8>().ok_or("a stream record is cut short")?;
            let stream = StreamId {
                opener,
                kind,
                n: u64::from_be_bytes(*n),
            };
            Ok(Record::Send(stream, bytes.to_vec()))
        }
        [1, 0] => Ok(Record::Close(Closing::Done)),
        [1, 1, refusal @ ..] => serde_json::from_slice(refusal)
            .map(|r| Record::Close(Closing::Refused(r)))
            .map_err(|e| format!("a refusal does not decode: {e}")),
        [1, 2] => Ok(Record::Close(Closing::Behind)),
        [1, 3, why @ ..] => Ok(Record::Close(Closing::Broken(
            String::from_utf8_lossy(why).into_owned(),
        ))),
        _ => Err("not a record".to_owned()),
    }
}

/// The next record of the pipe; none at its end between two records.
async fn read_record(reader: &mut (impl AsyncRead + Unpin)) -> Result<Option<Record>, String> {
    let mut head = [0u8; 4];
    match reader.read_exact(&mut head).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(format!("read the pipe: {e}")),
    }
    let len = usize::try_from(u32::from_be_bytes(head)).map_err(|e| e.to_string())?;
    if len > MAX_RECORD {
        return Err(format!("a record of {len} bytes is over {MAX_RECORD}"));
    }
    let mut body = vec![0u8; len];
    reader
        .read_exact(&mut body)
        .await
        .map_err(|e| format!("read the pipe: {e}"))?;
    decode(&body).map(Some)
}

/// What the `Conn` hands its transport in one `drive`, as records to write.
#[derive(Default)]
struct Outbox {
    records: Vec<Out>,
}

/// A record's bytes and, for a stream's, how many of the frame's bytes it flushes once written.
struct Out {
    bytes: Vec<u8>,
    written: Option<(StreamId, usize)>,
}

impl Transport for Outbox {
    fn open(&mut self, _: StreamId) {}

    fn send(&mut self, stream: StreamId, bytes: Vec<u8>) {
        let n = bytes.len();
        self.records.push(Out {
            bytes: encode(&Record::Send(stream, bytes)),
            written: Some((stream, n)),
        });
    }

    fn finish(&mut self, _: StreamId) {}

    fn reset(&mut self, _: StreamId) {}

    fn close(&mut self, closing: Closing) {
        self.records.push(Out {
            bytes: encode(&Record::Close(closing)),
            written: None,
        });
    }
}

type Answer = Box<dyn FnOnce(Result<Payload, Failure>) + Send>;
type Cmd = Box<dyn FnOnce(&mut Session) + Send>;

/// The `Conn` and what waits on it, owned by the session's task.
struct Session {
    conn: Conn,
    start: Instant,
    answers: HashMap<CallId, Answer>,
}

impl Session {
    fn now(&self) -> Millis {
        u64::try_from(self.start.elapsed().as_millis()).unwrap_or(Millis::MAX)
    }
}

fn gone() -> Failure {
    Failure::new(ErrorCode::Disconnected, "the session has ended")
}

/// A handle on a session: calls, pushes and the acceptor's say on a hello. Cheap to clone; every clone reaches the same session.
#[derive(Clone)]
pub struct Peer {
    cmds: mpsc::UnboundedSender<Cmd>,
}

/// A session that has a handle but does not run yet: `run` starts it.
pub struct Idle {
    peer: Peer,
    cmds: mpsc::UnboundedReceiver<Cmd>,
}

impl Peer {
    /// A handle, and the session it reaches once `Idle::run` starts it: what is asked before then waits for it.
    pub fn new() -> (Peer, Idle) {
        let (tx, cmds) = mpsc::unbounded_channel();
        let peer = Peer { cmds: tx };
        (peer.clone(), Idle { peer, cmds })
    }

    fn run(&self, cmd: impl FnOnce(&mut Session) + Send + 'static) -> bool {
        self.cmds.send(Box::new(cmd)).is_ok()
    }

    /// Makes a call and waits for its answer; a session that ended answers `DISCONNECTED`.
    pub async fn call<C, A>(&self, out: Outgoing<C, A>) -> Result<A, Failure>
    where
        C: Contract + Send + 'static,
        A: DeserializeOwned + Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        let sent = self.run(move |s| {
            let now = s.now();
            let pending = s.conn.call(now, out);
            let id = pending.id();
            let answer: Answer = Box::new(move |result| {
                tx.send(pending.decode(result)).ok();
            });
            s.answers.insert(id, answer);
        });
        if !sent {
            return Err(gone());
        }
        rx.await.unwrap_or_else(|_| Err(gone()))
    }

    /// Makes a call nobody waits for (an end the peer is told of); its answer is dropped.
    pub fn call_detached<C: Contract + Send + 'static, A: 'static>(&self, out: Outgoing<C, A>) {
        self.run(move |s| {
            let now = s.now();
            s.conn.call(now, out);
        });
    }

    /// Pushes on the stream of `home`, in order after the pushes before it there.
    pub async fn push<P: Pushes + Send + 'static>(&self, home: Option<String>, push: P) -> Result<(), Failure> {
        let (tx, rx) = oneshot::channel();
        let sent = self.run(move |s| {
            let now = s.now();
            tx.send(s.conn.push(now, home, &push)).ok();
        });
        if !sent {
            return Err(gone());
        }
        rx.await.unwrap_or_else(|_| Err(gone()))
    }

    /// The acceptor lets the dialler in, granting it the contracts it may call.
    pub fn admit(&self, granted: Vec<String>) {
        self.run(move |s| s.conn.admit(granted));
    }

    /// The acceptor turns the dialler away, which ends the session.
    pub fn refuse(&self, refusal: Refusal) {
        self.run(move |s| s.conn.refuse(refusal));
    }

    /// Ends the session on purpose: the pipe is closed after what was queued is written.
    pub fn close(&self) {
        self.run(|s| s.conn.closed(Closing::Done));
    }
}

/// What the writer tells the session: a stream's bytes reached the pipe, or the pipe broke.
enum Wrote {
    Flushed(StreamId, usize),
    Broke(String),
}

impl Idle {
    /// Runs `conn` over the pipe `reader`/`writer`, answering the peer's calls with `serve`, and returns the session's notes,
    /// the last `Closed`. It runs on the current tokio runtime until the session ends.
    pub fn run<R, W>(self, conn: Conn, reader: R, writer: W, serve: Serve) -> mpsc::UnboundedReceiver<Note>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let (notes_tx, notes) = mpsc::unbounded_channel();
        let (inbound_tx, inbound) = mpsc::unbounded_channel();
        let (out_tx, out_rx) = mpsc::unbounded_channel();
        let (wrote_tx, wrote) = mpsc::unbounded_channel();
        tokio::spawn(read_loop(reader, inbound_tx));
        tokio::spawn(write_loop(writer, out_rx, wrote_tx));
        let session = Session {
            conn,
            start: Instant::now(),
            answers: HashMap::new(),
        };
        let pipes = Pipes {
            cmds: self.cmds,
            inbound,
            wrote,
            out: out_tx,
            notes: notes_tx,
        };
        tokio::spawn(drive(session, pipes, serve, self.peer));
        notes
    }
}

async fn read_loop(mut reader: impl AsyncRead + Unpin, inbound: mpsc::UnboundedSender<Result<Record, String>>) {
    loop {
        match read_record(&mut reader).await {
            Ok(Some(record)) => {
                if inbound.send(Ok(record)).is_err() {
                    return;
                }
            }
            Ok(None) => return,
            Err(why) => {
                inbound.send(Err(why)).ok();
                return;
            }
        }
    }
}

async fn write_loop(
    mut writer: impl AsyncWrite + Unpin,
    mut out: mpsc::UnboundedReceiver<Out>,
    wrote: mpsc::UnboundedSender<Wrote>,
) {
    while let Some(record) = out.recv().await {
        let written = async {
            writer.write_all(&record.bytes).await?;
            writer.flush().await
        };
        if let Err(e) = written.await {
            wrote.send(Wrote::Broke(format!("write the pipe: {e}"))).ok();
            return;
        }
        if let Some((stream, n)) = record.written {
            wrote.send(Wrote::Flushed(stream, n)).ok();
        }
    }
    // The session ended: the peer reads the end of the pipe.
    writer.shutdown().await.ok();
}

struct Pipes {
    cmds: mpsc::UnboundedReceiver<Cmd>,
    inbound: mpsc::UnboundedReceiver<Result<Record, String>>,
    wrote: mpsc::UnboundedReceiver<Wrote>,
    out: mpsc::UnboundedSender<Out>,
    notes: mpsc::UnboundedSender<Note>,
}

/// Owns the session: hands the writer what the `Conn` sends, acts on its events, and feeds it the pipe, the commands of its
/// handles and the time. Returns once the session closed; dropping `out` then ends the writer, which closes the pipe.
async fn drive(mut s: Session, mut p: Pipes, serve: Serve, peer: Peer) {
    let mut reading = true;
    loop {
        let mut outbox = Outbox::default();
        let events = s.conn.drive(&mut outbox);
        for record in outbox.records {
            p.out.send(record).ok();
        }
        for event in events {
            match event {
                Event::Call {
                    id,
                    contract,
                    method,
                    body,
                } => {
                    let (serve, peer) = (Arc::clone(&serve), peer.clone());
                    tokio::spawn(async move {
                        // A handler that panics is answered, not left to time out.
                        let result = tokio::spawn(serve(contract, method, body)).await.unwrap_or_else(|e| {
                            Err(Failure::new(
                                ErrorCode::Aborted,
                                format!("the handler ended abnormally: {e}"),
                            ))
                        });
                        peer.run(move |s| s.conn.reply(id, result));
                    });
                }
                Event::Answer { id, result } => {
                    if let Some(answer) = s.answers.remove(&id) {
                        answer(result);
                    }
                }
                Event::Hello(hello) => {
                    p.notes.send(Note::Hello(hello)).ok();
                }
                Event::Welcomed(welcome) => {
                    p.notes.send(Note::Welcomed(welcome)).ok();
                }
                Event::Refused(refusal) => {
                    p.notes.send(Note::Refused(refusal)).ok();
                }
                Event::Push { contract, name, body } => {
                    p.notes.send(Note::Push { contract, name, body }).ok();
                }
                // The session closes next, with `Closing::Behind`.
                Event::Behind => {}
                Event::Closed(closing) => {
                    p.notes.send(Note::Closed(closing)).ok();
                    return;
                }
            }
        }
        let at = s
            .conn
            .next_deadline()
            .and_then(|ms| s.start.checked_add(Duration::from_millis(ms)));
        tokio::select! {
            Some(cmd) = p.cmds.recv() => cmd(&mut s),
            record = p.inbound.recv(), if reading => match record {
                Some(Ok(Record::Send(stream, bytes))) => s.conn.receive(stream, &bytes),
                Some(Ok(Record::Close(closing))) => s.conn.closed(closing),
                Some(Err(why)) => s.conn.closed(Closing::Broken(why)),
                None => {
                    reading = false;
                    s.conn.closed(Closing::Done);
                }
            },
            Some(wrote) = p.wrote.recv() => match wrote {
                Wrote::Flushed(stream, n) => s.conn.written(stream, n),
                Wrote::Broke(why) => s.conn.closed(Closing::Broken(why)),
            },
            () = tokio::time::sleep_until(at.unwrap_or(s.start)), if at.is_some() => {
                let now = s.now();
                s.conn.tick(now);
            }
        }
    }
}
