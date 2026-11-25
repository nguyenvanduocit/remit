//! One session's task: it owns the `Conn`, feeds it what the streams read and the clock, carries out what it hands its
//! `Transport`, settles the calls this side made and passes every other event on.

use std::collections::HashMap;
use std::time::Duration;

use link::{
    CallId, Closing, Conn, Contract, ErrorCode, Event, Failure, Incoming, Millis, Outgoing, Payload, Pending, Pushes,
    Refusal, Side, Transport,
};
use serde::de::DeserializeOwned;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;
use web_transport_quinn::Session;

use crate::streams::{Happened, Streams};

/// What the session task is asked to do by a `Link`.
enum Command {
    Call(MakeCall),
    Push(PushTo, oneshot::Sender<Result<(), Failure>>),
    Reply(Incoming, Result<Payload, Failure>),
    Admit(Vec<String>),
    Refuse(Refusal),
    Close,
}

/// Hands a call's answer to whoever awaits it, decoded as the call's own type.
type Settle = Box<dyn FnOnce(Result<Payload, Failure>) + Send>;

/// Makes a call on the conn, typed by its contract, and says how to settle its answer.
type MakeCall = Box<dyn FnOnce(&mut Conn, Millis) -> (CallId, Settle) + Send>;

/// Pushes on the conn, typed by its contract.
type PushTo = Box<dyn FnOnce(&mut Conn, Millis) -> Result<(), Failure> + Send>;

/// One peer's session: `link` to act on it, `events` for what the peer does (hello, welcome, calls to `reply` to,
/// pushes, closing). Answers to this side's calls are not among the events: `Link::call` returns them.
pub struct Peer {
    pub link: Link,
    pub events: mpsc::UnboundedReceiver<Event>,
    /// Where the other side's packets come from, as the QUIC connection sees it.
    pub remote: std::net::SocketAddr,
}

/// A handle on one session; the session closes when the last handle is dropped.
#[derive(Clone)]
pub struct Link {
    commands: mpsc::UnboundedSender<Command>,
}

fn gone() -> Failure {
    Failure::new(ErrorCode::Disconnected, "the session ended before the answer")
}

impl Link {
    /// Calls the peer and waits for the answer, or for why there is none (`TIMEOUT`, `DISCONNECTED`, the peer's code).
    pub async fn call<C, A>(&self, out: Outgoing<C, A>) -> Result<A, Failure>
    where
        C: Contract + Send + 'static,
        A: DeserializeOwned + Send + 'static,
    {
        self.send(out, |pending, result| pending.decode(result)).await
    }

    /// Calls the peer and waits for the answer still encoded: for a caller that names the call only at run time and
    /// decodes it with its contract's `decode_answer`.
    pub async fn call_encoded<C>(&self, out: Outgoing<C, Payload>) -> Result<Payload, Failure>
    where
        C: Contract + Send + 'static,
    {
        self.send(out, |_, result| result).await
    }

    async fn send<C, A, R>(
        &self,
        out: Outgoing<C, A>,
        finish: impl FnOnce(&Pending<A>, Result<Payload, Failure>) -> Result<R, Failure> + Send + 'static,
    ) -> Result<R, Failure>
    where
        C: Contract + Send + 'static,
        A: Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        let call = move |conn: &mut Conn, now: Millis| {
            let pending = conn.call(now, out);
            let id = pending.id();
            let settle: Settle = Box::new(move |result| {
                // The caller stopped waiting: nobody is left to tell.
                tx.send(finish(&pending, result)).ok();
            });
            (id, settle)
        };
        if self.commands.send(Command::Call(Box::new(call))).is_err() {
            return Err(gone());
        }
        rx.await.unwrap_or_else(|_| Err(gone()))
    }

    /// Pushes on the stream of `home` (none: the feed), in order after the pushes before it there.
    pub async fn push<P: Pushes + Send + 'static>(&self, home: Option<String>, push: P) -> Result<(), Failure> {
        let (tx, rx) = oneshot::channel();
        let send = move |conn: &mut Conn, now: Millis| conn.push(now, home, &push);
        if self.commands.send(Command::Push(Box::new(send), tx)).is_err() {
            return Err(gone());
        }
        rx.await.unwrap_or_else(|_| Err(gone()))
    }

    /// Answers a call the peer made (`Event::Call`).
    pub fn reply(&self, id: Incoming, result: Result<Payload, Failure>) {
        self.tell(Command::Reply(id, result));
    }

    /// Lets the dialler in after its `Event::Hello`, granting it these contracts.
    pub fn admit(&self, granted: Vec<String>) {
        self.tell(Command::Admit(granted));
    }

    /// Turns the dialler away, and closes the session.
    pub fn refuse(&self, refusal: Refusal) {
        self.tell(Command::Refuse(refusal));
    }

    pub fn close(&self) {
        self.tell(Command::Close);
    }

    /// A command for a session already ended has nothing left to act on.
    fn tell(&self, command: Command) {
        self.commands.send(command).ok();
    }
}

/// Starts the task of an open session whose other side is `peer`.
pub(crate) fn start(session: Session, conn: Conn, peer: Side) -> Peer {
    let streams = Streams::start(session, peer);
    let remote = streams.remote_address();
    let (commands, commands_rx) = mpsc::unbounded_channel();
    let (events_tx, events) = mpsc::unbounded_channel();
    let driver = Driver {
        conn,
        streams,
        start: Instant::now(),
        settles: HashMap::new(),
        events: events_tx,
        ended: false,
    };
    tokio::spawn(driver.run(commands_rx));
    Peer {
        link: Link { commands },
        events,
        remote,
    }
}

struct Driver {
    conn: Conn,
    streams: Streams,
    start: Instant,
    settles: HashMap<CallId, Settle>,
    events: mpsc::UnboundedSender<Event>,
    ended: bool,
}

impl Driver {
    async fn run(mut self, mut commands: mpsc::UnboundedReceiver<Command>) {
        loop {
            self.drive();
            if self.ended {
                break;
            }
            let deadline = self.conn.next_deadline().map(|ms| self.instant(ms));
            tokio::select! {
                command = commands.recv() => match command {
                    Some(command) => self.command(command),
                    None => self.close_on_purpose(),
                },
                happened = self.streams.next() => self.happened(happened),
                () = sleep_until(deadline) => {
                    let now = self.now();
                    self.conn.tick(now);
                }
            }
        }
        // Calls that never reached the conn learn the session went when their settles are dropped; dropping the
        // streams ends their tasks.
    }

    fn now(&self) -> Millis {
        u64::try_from(self.start.elapsed().as_millis()).unwrap_or(Millis::MAX)
    }

    fn instant(&self, ms: Millis) -> Instant {
        self.start.checked_add(Duration::from_millis(ms)).unwrap_or(self.start)
    }

    fn command(&mut self, command: Command) {
        let now = self.now();
        match command {
            Command::Call(call) => {
                let (id, settle) = call(&mut self.conn, now);
                self.settles.insert(id, settle);
            }
            Command::Push(push, done) => {
                done.send(push(&mut self.conn, now)).ok();
            }
            Command::Reply(id, result) => self.conn.reply(id, result),
            Command::Admit(granted) => self.conn.admit(granted),
            Command::Refuse(refusal) => self.conn.refuse(refusal),
            Command::Close => self.close_on_purpose(),
        }
    }

    fn close_on_purpose(&mut self) {
        self.streams.close(Closing::Done);
        self.conn.closed(Closing::Done);
    }

    fn happened(&mut self, happened: Happened) {
        match happened {
            Happened::Read(id, bytes) => self.conn.receive(id, &bytes),
            Happened::Written(id, n) => self.conn.written(id, n),
            Happened::Closed(closing) => self.conn.closed(closing),
        }
    }

    /// Carries out what the conn has to do and routes what happened.
    fn drive(&mut self) {
        let events = self.conn.drive(&mut self.streams);
        for event in events {
            match event {
                Event::Answer { id, result } => {
                    if let Some(settle) = self.settles.remove(&id) {
                        settle(result);
                    }
                }
                Event::Closed(closing) => {
                    self.ended = true;
                    self.events.send(Event::Closed(closing)).ok();
                }
                other => {
                    // Nobody reads the events: the session still answers calls and settles its own.
                    self.events.send(other).ok();
                }
            }
        }
    }
}

async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}
