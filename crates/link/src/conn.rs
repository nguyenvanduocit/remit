//! One session between two peers, with no I/O: either half calls and pushes, and both keep the five guarantees. A driver
//! feeds it what its transport saw (`receive`, `written`, `closed`) and the time (`tick`), does what `drive` hands its
//! `Transport`, and acts on the `Event`s `drive` returns.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::mem;

use crate::contract::{Contract, Outgoing, Pending, Pushes};
use crate::envelope::{ErrorCode, Failure, Frame, Hello, Node, Payload, Reader, Refusal, RefusalCode, Welcome, encode};

/// Milliseconds on the driver's clock: the session reads no clock of its own.
pub type Millis = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// How long a call waits for its answer unless it declared its own timeout; also how long a push may wait unsent.
    pub call_timeout: Millis,
    /// Unsent push bytes on one stream past which the peer is behind.
    pub behind_bytes: usize,
    /// How often a live half pings on the control stream. A transport drops a session that carries nothing for a while
    /// (QUIC's idle timeout: 30 s by default in quinn, `TransportConfig::max_idle_timeout`), so this stays well under it.
    pub ping_every: Millis,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            call_timeout: 60_000,
            behind_bytes: 4 << 20,
            ping_every: 10_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Side {
    Dialer,
    Acceptor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StreamKind {
    Bi,
    Uni,
}

/// A stream, named by who opened it, its kind and its number among that peer's streams; both halves name it the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StreamId {
    pub opener: Side,
    pub kind: StreamKind,
    pub n: u64,
}

/// Why a session ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Closing {
    /// Closed on purpose, or the transport went.
    Done,
    /// The hello was turned away.
    Refused(Refusal),
    /// The pushing side gave up on a peer that read too slowly: it reconnects and re-reads its snapshot.
    Behind,
    /// The control stream carried what is not a frame.
    Broken(String),
}

// A session's close code says why it ended, so the peer knows even when the frame that said so on the control stream
// was lost with the connection. Every driver closes and reads a session by these.
const DONE: u32 = 0;
const BEHIND: u32 = 1;
const BROKEN: u32 = 2;
const BAD_CREDENTIAL: u32 = 3;
const NEW_BUNDLE: u32 = 4;

impl Closing {
    /// The code and reason a driver closes its session with.
    pub fn code(&self) -> (u32, &str) {
        match self {
            Self::Done => (DONE, ""),
            Self::Behind => (BEHIND, ""),
            Self::Broken(why) => (BROKEN, why),
            Self::Refused(r) => match r.code {
                RefusalCode::BadCredential => (BAD_CREDENTIAL, &r.reason),
                RefusalCode::NewBundle => (NEW_BUNDLE, &r.reason),
            },
        }
    }

    /// Why a session the peer closed with `code` and `reason` ended; a code no driver writes is a session done.
    pub fn from_code(code: u32, reason: &str) -> Self {
        let refused = |code| {
            Self::Refused(Refusal {
                code,
                reason: reason.to_owned(),
            })
        };
        match code {
            BEHIND => Self::Behind,
            BROKEN => Self::Broken(reason.to_owned()),
            BAD_CREDENTIAL => refused(RefusalCode::BadCredential),
            NEW_BUNDLE => refused(RefusalCode::NewBundle),
            _ => Self::Done,
        }
    }
}

/// What a driver does on its session (a WebTransport session, a WebSocket carrying stream ids, a pipe in a test).
pub trait Transport {
    fn open(&mut self, stream: StreamId);
    fn send(&mut self, stream: StreamId, bytes: Vec<u8>);
    /// Ends the sending side of a stream.
    fn finish(&mut self, stream: StreamId);
    fn reset(&mut self, stream: StreamId);
    fn close(&mut self, closing: Closing);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CallId(pub u64);

/// A call the peer made, waiting for `reply`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Incoming(pub u64);

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// The acceptor's: a peer says hello; `admit` or `refuse` it.
    Hello(Hello),
    /// The dialler's: admitted.
    Welcomed(Welcome),
    Refused(Refusal),
    /// The peer calls; answer with `reply`. Calls of one line (`Contract::order`) come one at a time, in the order sent.
    Call {
        id: Incoming,
        contract: String,
        method: String,
        body: Payload,
    },
    /// The answer to a call this half made, or why it has none.
    Answer {
        id: CallId,
        result: Result<Payload, Failure>,
    },
    Push {
        contract: String,
        name: String,
        body: Payload,
    },
    /// The pusher says this half fell behind; the session closes next.
    Behind,
    Closed(Closing),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Greeting,
    Live,
    Closed,
}

#[derive(Debug)]
enum Io {
    Open(StreamId),
    Send(StreamId, Vec<u8>),
    Finish(StreamId),
    Reset(StreamId),
    Close(Closing),
}

#[derive(Debug)]
struct OutCall {
    stream: StreamId,
    deadline: Millis,
}

#[derive(Debug)]
struct Waiting {
    stream: StreamId,
    home: Option<String>,
    method: String,
    body: Payload,
}

/// The calls of one line the peer sent: the next `seq` to run, whether one runs, and those that came early.
#[derive(Debug, Default)]
struct Queue {
    next: u64,
    busy: bool,
    waiting: BTreeMap<u64, Waiting>,
}

/// Which calls of a contract wait their turn together: those of one method, or the changes naming one entity (`order`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Turn {
    Method(String),
    Order(String),
}

impl Turn {
    fn of(method: &str, order: Option<&str>) -> Self {
        order.map_or_else(|| Self::Method(method.to_owned()), |o| Self::Order(o.to_owned()))
    }
}

/// A contract and one line of its calls.
type Line = (String, Turn);

#[derive(Debug)]
struct Unsent {
    bytes: usize,
    since: Millis,
}

#[derive(Debug)]
pub struct Conn {
    cfg: Config,
    me: Node,
    side: Side,
    phase: Phase,
    control: Option<StreamId>,
    next_stream: u64,
    next_call: u64,
    next_incoming: u64,
    /// The contracts the peer may call here: granted at `welcome` (acceptor) or offered at `hello` (dialler).
    peer_may_call: Vec<String>,
    readers: HashMap<StreamId, Reader>,
    out_calls: BTreeMap<CallId, OutCall>,
    by_stream: HashMap<StreamId, CallId>,
    sent_seq: HashMap<(&'static str, Turn), u64>,
    queues: HashMap<Line, Queue>,
    incoming: HashMap<Incoming, (StreamId, Line)>,
    push_streams: HashMap<Option<String>, StreamId>,
    unsent: HashMap<StreamId, Unsent>,
    /// When this half last pinged.
    pinged_at: Millis,
    io: VecDeque<Io>,
    events: Vec<Event>,
}

impl Conn {
    fn new(cfg: Config, me: Node, side: Side, peer_may_call: Vec<String>) -> Self {
        Self {
            cfg,
            me,
            side,
            phase: Phase::Greeting,
            control: None,
            next_stream: 0,
            next_call: 0,
            next_incoming: 0,
            peer_may_call,
            readers: HashMap::new(),
            out_calls: BTreeMap::new(),
            by_stream: HashMap::new(),
            sent_seq: HashMap::new(),
            queues: HashMap::new(),
            incoming: HashMap::new(),
            push_streams: HashMap::new(),
            unsent: HashMap::new(),
            pinged_at: 0,
            io: VecDeque::new(),
            events: Vec::new(),
        }
    }

    /// The dialling half: opens the control stream and says hello on it.
    pub fn dial(cfg: Config, hello: Hello) -> Self {
        let mut conn = Self::new(cfg, hello.node.clone(), Side::Dialer, hello.offers.clone());
        let control = conn.open(StreamKind::Bi);
        conn.control = Some(control);
        conn.send_frame(control, &Frame::Hello(hello));
        conn
    }

    /// The accepting half: it reads nothing but a hello until it admits one.
    pub fn accept(cfg: Config, me: Node) -> Self {
        Self::new(cfg, me, Side::Acceptor, Vec::new())
    }

    pub fn is_live(&self) -> bool {
        self.phase == Phase::Live
    }

    /// Lets the dialler in, granting it the contracts it may call.
    pub fn admit(&mut self, granted: Vec<String>) {
        let Some(control) = self.control else { return };
        if self.side != Side::Acceptor || self.phase != Phase::Greeting {
            return;
        }
        self.peer_may_call.clone_from(&granted);
        self.send_frame(
            control,
            &Frame::Welcome(Welcome {
                node: self.me.clone(),
                granted,
            }),
        );
        self.phase = Phase::Live;
    }

    /// Turns the dialler away with why, and closes the session.
    pub fn refuse(&mut self, refusal: Refusal) {
        if let Some(control) = self.control {
            self.send_frame(control, &Frame::Refused(refusal.clone()));
        }
        self.close(Closing::Refused(refusal));
    }

    /// Bytes the transport read on a stream.
    pub fn receive(&mut self, stream: StreamId, bytes: &[u8]) {
        if self.phase == Phase::Closed {
            return;
        }
        self.readers.entry(stream).or_default().push(bytes);
        while let Some(next) = self.readers.get_mut(&stream).and_then(Reader::next_frame) {
            match next {
                Ok(frame) => self.on_frame(stream, frame),
                Err(e) => self.on_garbage(stream, &e.to_string()),
            }
        }
    }

    /// The transport flushed `n` bytes of a stream to the peer.
    pub fn written(&mut self, stream: StreamId, n: usize) {
        if let Some(u) = self.unsent.get_mut(&stream) {
            u.bytes = u.bytes.saturating_sub(n);
            if u.bytes == 0 {
                self.unsent.remove(&stream);
            }
        }
    }

    /// The transport's session ended under this half.
    pub fn closed(&mut self, closing: Closing) {
        self.ended(closing);
    }

    /// Time passed: calls past their timeout fail `TIMEOUT`, a peer that reads too slowly is let go, and a live session
    /// is pinged when its ping is due.
    pub fn tick(&mut self, now: Millis) {
        let expired: Vec<CallId> = self
            .out_calls
            .iter()
            .filter(|(_, c)| c.deadline <= now)
            .map(|(id, _)| *id)
            .collect();
        for id in expired {
            if let Some(call) = self.out_calls.remove(&id) {
                self.by_stream.remove(&call.stream);
                self.readers.remove(&call.stream);
                self.io.push_back(Io::Reset(call.stream));
                self.events.push(Event::Answer {
                    id,
                    result: Err(Failure::new(ErrorCode::Timeout, "no answer in time")),
                });
            }
        }
        self.check_behind(now);
        self.ping(now);
    }

    /// When `tick` has something to do next.
    pub fn next_deadline(&self) -> Option<Millis> {
        let calls = self.out_calls.values().map(|c| c.deadline);
        let pushes = self
            .unsent
            .values()
            .map(|u| u.since.saturating_add(self.cfg.call_timeout));
        calls.chain(pushes).chain(self.ping_due()).min()
    }

    fn ping_due(&self) -> Option<Millis> {
        self.is_live()
            .then(|| self.pinged_at.saturating_add(self.cfg.ping_every))
    }

    /// Keeps the session's transport from closing it as idle; the peer's `pong` carries traffic back the other way.
    fn ping(&mut self, now: Millis) {
        let (Some(due), Some(control)) = (self.ping_due(), self.control) else {
            return;
        };
        if due <= now {
            self.send_frame(control, &Frame::Ping);
            self.pinged_at = now;
        }
    }

    /// Sends a call; its answer, or why there is none, comes as an `Event::Answer` naming the returned id. With no live
    /// session nothing is sent and the call fails `DISCONNECTED` at once.
    pub fn call<C: Contract, A>(&mut self, now: Millis, out: Outgoing<C, A>) -> Pending<A> {
        let id = CallId(self.next_call);
        self.next_call = self.next_call.wrapping_add(1);
        if self.phase != Phase::Live {
            let failure = Failure::new(ErrorCode::Disconnected, "no session: nothing was sent");
            self.events.push(Event::Answer {
                id,
                result: Err(failure),
            });
            return Pending::new(id);
        }
        let (call, home) = out.into_parts();
        let body = match call.encode_args() {
            Ok(body) => body,
            Err(failure) => {
                self.events.push(Event::Answer {
                    id,
                    result: Err(failure),
                });
                return Pending::new(id);
            }
        };
        let order = call.order().map(str::to_owned);
        let counter = self
            .sent_seq
            .entry((C::CAPABILITY, Turn::of(call.method(), order.as_deref())))
            .or_insert(0);
        let seq = *counter;
        *counter = counter.wrapping_add(1);
        let stream = self.open(StreamKind::Bi);
        self.send_frame(
            stream,
            &Frame::Call {
                seq,
                order,
                contract: C::CAPABILITY.to_owned(),
                method: call.method().to_owned(),
                home,
                body,
            },
        );
        self.io.push_back(Io::Finish(stream));
        let deadline = now.saturating_add(call.timeout_ms().unwrap_or(self.cfg.call_timeout));
        self.out_calls.insert(id, OutCall { stream, deadline });
        self.by_stream.insert(stream, id);
        Pending::new(id)
    }

    /// Pushes on the stream of `home` (a chain's or the Assistant's id; none: the feed), in order after the pushes before
    /// it there. A peer whose stream holds too much unsent, or for too long, is sent `behind` and the session closes.
    pub fn push<P: Pushes>(&mut self, now: Millis, home: Option<String>, push: &P) -> Result<(), Failure> {
        if self.phase != Phase::Live {
            return Err(Failure::new(ErrorCode::Disconnected, "no session"));
        }
        let body = push.encode_args()?;
        let stream = match self.push_streams.get(&home) {
            Some(stream) => *stream,
            None => {
                let stream = self.open(StreamKind::Uni);
                self.push_streams.insert(home, stream);
                stream
            }
        };
        let n = self.send_frame(
            stream,
            &Frame::Push {
                contract: P::CAPABILITY.to_owned(),
                name: push.name().to_owned(),
                body,
            },
        );
        let unsent = self.unsent.entry(stream).or_insert(Unsent { bytes: 0, since: now });
        unsent.bytes = unsent.bytes.saturating_add(n);
        self.check_behind(now);
        Ok(())
    }

    /// Answers a call the peer made; the next call of its line, if one waits, comes next.
    pub fn reply(&mut self, id: Incoming, result: Result<Payload, Failure>) {
        let Some((stream, line)) = self.incoming.remove(&id) else {
            return;
        };
        if self.phase == Phase::Closed {
            return;
        }
        self.answer(stream, result);
        if let Some(queue) = self.queues.get_mut(&line) {
            queue.busy = false;
        }
        self.pump(&line);
    }

    /// Hands the transport what this half has to do, and returns what happened.
    pub fn drive(&mut self, transport: &mut impl Transport) -> Vec<Event> {
        while let Some(io) = self.io.pop_front() {
            match io {
                Io::Open(stream) => transport.open(stream),
                Io::Send(stream, bytes) => transport.send(stream, bytes),
                Io::Finish(stream) => transport.finish(stream),
                Io::Reset(stream) => transport.reset(stream),
                Io::Close(closing) => transport.close(closing),
            }
        }
        mem::take(&mut self.events)
    }

    fn open(&mut self, kind: StreamKind) -> StreamId {
        let stream = StreamId {
            opener: self.side,
            kind,
            n: self.next_stream,
        };
        self.next_stream = self.next_stream.wrapping_add(1);
        self.io.push_back(Io::Open(stream));
        stream
    }

    /// Writes a frame and says how many bytes it took; a frame that cannot be encoded resets its stream.
    fn send_frame(&mut self, stream: StreamId, frame: &Frame) -> usize {
        match encode(frame) {
            Ok(bytes) => {
                let n = bytes.len();
                self.io.push_back(Io::Send(stream, bytes));
                n
            }
            Err(_) => {
                self.io.push_back(Io::Reset(stream));
                0
            }
        }
    }

    fn answer(&mut self, stream: StreamId, result: Result<Payload, Failure>) {
        let frame = match result {
            Ok(value) => Frame::Ok { value },
            Err(failure) => Frame::Err(failure),
        };
        self.send_frame(stream, &frame);
        self.io.push_back(Io::Finish(stream));
        self.readers.remove(&stream);
    }

    fn refuse_stream(&mut self, stream: StreamId) {
        self.readers.remove(&stream);
        self.io.push_back(Io::Reset(stream));
    }

    fn on_garbage(&mut self, stream: StreamId, why: &str) {
        self.readers.remove(&stream);
        if self.control.is_none() && self.side == Side::Acceptor && stream.opener != self.side {
            // A first frame that does not decode is a peer of another build (a page left open across a release, a plugin
            // built before it): told by the close code, which no frame format can hide, to load itself again.
            self.close(Closing::Refused(Refusal {
                code: RefusalCode::NewBundle,
                reason: format!("the hello does not decode, so the peer is of another build: {why}"),
            }));
        } else if Some(stream) == self.control || stream.kind == StreamKind::Uni {
            // A control or push stream that lost a frame cannot be trusted to carry the rest in order.
            self.close(Closing::Broken(format!("a frame does not decode: {why}")));
        } else if stream.opener != self.side && self.phase == Phase::Live {
            self.answer(
                stream,
                Err(Failure::new(ErrorCode::InvalidArgument, format!("not a frame: {why}"))),
            );
        } else if let Some(id) = self.by_stream.remove(&stream) {
            self.out_calls.remove(&id);
            let failure = Failure::new(ErrorCode::InternalError, format!("the answer is not a frame: {why}"));
            self.events.push(Event::Answer {
                id,
                result: Err(failure),
            });
        }
    }

    fn on_frame(&mut self, stream: StreamId, frame: Frame) {
        let peer = stream.opener != self.side;
        let Some(control) = self.control else {
            // The acceptor before hello: only a hello on the dialler's first stream is read.
            match frame {
                Frame::Hello(hello) if peer && stream.kind == StreamKind::Bi => {
                    self.control = Some(stream);
                    self.events.push(Event::Hello(hello));
                }
                _ => self.refuse_stream(stream),
            }
            return;
        };
        let on_control = stream == control;
        if self.phase == Phase::Greeting && !on_control {
            self.refuse_stream(stream);
            return;
        }
        match frame {
            Frame::Welcome(welcome) if on_control && self.side == Side::Dialer => {
                self.phase = Phase::Live;
                self.events.push(Event::Welcomed(welcome));
            }
            Frame::Refused(refusal) if on_control => self.events.push(Event::Refused(refusal)),
            Frame::Behind if on_control => self.events.push(Event::Behind),
            Frame::Ping if on_control => {
                self.send_frame(control, &Frame::Pong);
            }
            Frame::Hello(_) | Frame::Pong if on_control => {}
            Frame::Call {
                seq,
                order,
                contract,
                method,
                home,
                body,
            } if peer && stream.kind == StreamKind::Bi => {
                let key = (contract, Turn::of(&method, order.as_deref()));
                let waiting = Waiting {
                    stream,
                    home,
                    method,
                    body,
                };
                self.enqueue(stream, seq, key, waiting);
            }
            Frame::Push { contract, name, body } if peer && stream.kind == StreamKind::Uni => {
                self.events.push(Event::Push { contract, name, body });
            }
            Frame::Ok { value } if !peer => self.settle(stream, Ok(value)),
            Frame::Err(failure) if !peer => self.settle(stream, Err(failure)),
            _ => self.refuse_stream(stream),
        }
    }

    fn enqueue(&mut self, stream: StreamId, seq: u64, key: Line, waiting: Waiting) {
        let queue = self.queues.entry(key.clone()).or_default();
        if seq < queue.next || queue.waiting.contains_key(&seq) {
            self.answer(
                stream,
                Err(Failure::new(
                    ErrorCode::InvalidArgument,
                    "a call's seq was used already",
                )),
            );
            return;
        }
        queue.waiting.insert(seq, waiting);
        self.pump(&key);
    }

    /// Runs the next call of a line once the one before it was answered, in `seq` order whatever order they arrived in.
    fn pump(&mut self, key: &Line) {
        loop {
            let Some(queue) = self.queues.get_mut(key) else { return };
            if queue.busy {
                return;
            }
            let Some(waiting) = queue.waiting.remove(&queue.next) else {
                return;
            };
            queue.next = queue.next.wrapping_add(1);
            if !self.peer_may_call.contains(&key.0) {
                let failure = Failure::new(ErrorCode::Forbidden, format!("{} was not granted", key.0));
                self.answer(waiting.stream, Err(failure));
                continue;
            }
            if let Some(home) = &waiting.home
                && *home != self.me.id
            {
                let failure = Failure::new(ErrorCode::NotHere, format!("{home} is another node"));
                self.answer(waiting.stream, Err(failure));
                continue;
            }
            if let Some(queue) = self.queues.get_mut(key) {
                queue.busy = true;
            }
            let id = Incoming(self.next_incoming);
            self.next_incoming = self.next_incoming.wrapping_add(1);
            self.incoming.insert(id, (waiting.stream, key.clone()));
            self.events.push(Event::Call {
                id,
                contract: key.0.clone(),
                method: waiting.method,
                body: waiting.body,
            });
            return;
        }
    }

    /// An answer on a stream of this half's: delivered once; one for a call already timed out is dropped.
    fn settle(&mut self, stream: StreamId, result: Result<Payload, Failure>) {
        self.readers.remove(&stream);
        if let Some(id) = self.by_stream.remove(&stream) {
            self.out_calls.remove(&id);
            self.events.push(Event::Answer { id, result });
        }
    }

    fn check_behind(&mut self, now: Millis) {
        if self.phase != Phase::Live {
            return;
        }
        let behind = self
            .unsent
            .values()
            .any(|u| u.bytes > self.cfg.behind_bytes || now.saturating_sub(u.since) >= self.cfg.call_timeout);
        if behind {
            if let Some(control) = self.control {
                self.send_frame(control, &Frame::Behind);
            }
            self.close(Closing::Behind);
        }
    }

    fn close(&mut self, closing: Closing) {
        if self.phase == Phase::Closed {
            return;
        }
        self.io.push_back(Io::Close(closing.clone()));
        self.ended(closing);
    }

    /// Every call still out fails `DISCONNECTED`: whether the peer acted is not known, so a changing call is retried
    /// with its same `Request`.
    fn ended(&mut self, closing: Closing) {
        if self.phase == Phase::Closed {
            return;
        }
        self.phase = Phase::Closed;
        for (id, _) in mem::take(&mut self.out_calls) {
            let failure = Failure::new(ErrorCode::Disconnected, "the session ended before the answer");
            self.events.push(Event::Answer {
                id,
                result: Err(failure),
            });
        }
        self.by_stream.clear();
        self.incoming.clear();
        self.queues.clear();
        self.unsent.clear();
        self.events.push(Event::Closed(closing));
    }
}
