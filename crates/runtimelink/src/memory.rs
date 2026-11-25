//! Both halves of one link session in this process, on tokio: `link::memory::Pair` driven by one task, which settles each
//! half's calls and hands every other event to that half's `events`. It stands in for `link-quic` in the tests of the
//! core and of the runtime.

use std::collections::HashMap;
use std::mem;
use std::time::Duration;

use link::memory::Pair;
use link::{
    CallId, Closing, Config, Conn, Contract, ErrorCode, Event, Failure, Hello, Incoming, Millis, Node, Outgoing,
    Payload, Refusal, Side,
};
use serde::de::DeserializeOwned;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

use crate::port::{self, Peer};

/// Hands a call's answer to whoever awaits it, decoded as the call's own type.
type Settle = Box<dyn FnOnce(Result<Payload, Failure>) + Send>;

/// Makes a call on a half, typed by its contract, and says how to settle its answer.
type MakeCall = Box<dyn FnOnce(&mut Conn, Millis) -> (CallId, Settle) + Send>;

enum Command {
    Call(MakeCall),
    Reply(Incoming, Result<Payload, Failure>),
    Admit(Vec<String>),
    Refuse(Refusal),
    Close,
}

/// A handle on one half; the session closes when every handle on either half is dropped, or on `close`.
#[derive(Clone)]
pub struct Link {
    commands: mpsc::UnboundedSender<Command>,
}

fn gone() -> Failure {
    Failure::new(ErrorCode::Disconnected, "the session ended before the answer")
}

impl Link {
    /// A command for a session already ended has nothing left to act on.
    fn tell(&self, command: Command) {
        self.commands.send(command).ok();
    }
}

impl port::Link for Link {
    async fn call<C, A>(&self, out: Outgoing<C, A>) -> Result<A, Failure>
    where
        C: Contract + Send + 'static,
        A: DeserializeOwned + Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        let call: MakeCall = Box::new(move |conn: &mut Conn, now: Millis| {
            let pending = conn.call(now, out);
            let id = pending.id();
            let settle: Settle = Box::new(move |result| {
                // The caller stopped waiting: nobody is left to tell.
                tx.send(pending.decode(result)).ok();
            });
            (id, settle)
        });
        if self.commands.send(Command::Call(call)).is_err() {
            return Err(gone());
        }
        rx.await.unwrap_or_else(|_| Err(gone()))
    }

    fn reply(&self, id: Incoming, result: Result<Payload, Failure>) {
        self.tell(Command::Reply(id, result));
    }

    fn admit(&self, granted: Vec<String>) {
        self.tell(Command::Admit(granted));
    }

    fn refuse(&self, refusal: Refusal) {
        self.tell(Command::Refuse(refusal));
    }

    fn close(&self) {
        self.tell(Command::Close);
    }
}

/// A session between a dialler that says `hello` and an acceptor that is `acceptor`: the dialler's half first. The
/// acceptor's first event is the hello, to `admit` or `refuse`. Needs a tokio runtime.
pub fn pair(cfg: Config, hello: Hello, acceptor: Node) -> (Peer<Link>, Peer<Link>) {
    let (dialer_commands, dialer_rx) = mpsc::unbounded_channel();
    let (acceptor_commands, acceptor_rx) = mpsc::unbounded_channel();
    let (dialer_events, dialer_events_rx) = mpsc::unbounded_channel();
    let (acceptor_events, acceptor_events_rx) = mpsc::unbounded_channel();
    let driver = Driver {
        start: Instant::now(),
        pair: Pair::new(Conn::dial(cfg, hello), Conn::accept(cfg, acceptor)),
        settles: HashMap::new(),
        dialer: Half {
            events: dialer_events,
            closed: false,
        },
        acceptor: Half {
            events: acceptor_events,
            closed: false,
        },
    };
    tokio::spawn(driver.run(dialer_rx, acceptor_rx));
    let dialer = Peer {
        link: Link {
            commands: dialer_commands,
        },
        events: dialer_events_rx,
    };
    let acceptor = Peer {
        link: Link {
            commands: acceptor_commands,
        },
        events: acceptor_events_rx,
    };
    (dialer, acceptor)
}

struct Half {
    events: mpsc::UnboundedSender<Event>,
    closed: bool,
}

struct Driver {
    start: Instant,
    pair: Pair,
    settles: HashMap<(Side, CallId), Settle>,
    dialer: Half,
    acceptor: Half,
}

impl Driver {
    async fn run(
        mut self,
        mut dialer: mpsc::UnboundedReceiver<Command>,
        mut acceptor: mpsc::UnboundedReceiver<Command>,
    ) {
        let (mut dialer_held, mut acceptor_held) = (true, true);
        loop {
            self.route();
            if self.dialer.closed && self.acceptor.closed {
                return;
            }
            let deadline = self.deadline();
            tokio::select! {
                command = dialer.recv(), if dialer_held => match command {
                    Some(command) => self.command(Side::Dialer, command),
                    None => {
                        dialer_held = false;
                        self.close();
                    }
                },
                command = acceptor.recv(), if acceptor_held => match command {
                    Some(command) => self.command(Side::Acceptor, command),
                    None => {
                        acceptor_held = false;
                        self.close();
                    }
                },
                () = sleep_until(deadline) => self.catch_up(),
            }
        }
    }

    fn now(&self) -> Millis {
        u64::try_from(self.start.elapsed().as_millis()).unwrap_or(Millis::MAX)
    }

    /// Moves the pair's clock to now, so both halves act on the time that passed.
    fn catch_up(&mut self) {
        let by = self.now().saturating_sub(self.pair.now);
        self.pair.advance(by);
    }

    fn deadline(&self) -> Option<Instant> {
        let next = [self.pair.dialer.next_deadline(), self.pair.acceptor.next_deadline()];
        let ms = next.into_iter().flatten().min()?;
        Some(self.start.checked_add(Duration::from_millis(ms)).unwrap_or(self.start))
    }

    fn command(&mut self, side: Side, command: Command) {
        self.catch_up();
        let now = self.pair.now;
        let conn = match side {
            Side::Dialer => &mut self.pair.dialer,
            Side::Acceptor => &mut self.pair.acceptor,
        };
        match command {
            Command::Call(call) => {
                let (id, settle) = call(conn, now);
                self.settles.insert((side, id), settle);
            }
            Command::Reply(id, result) => conn.reply(id, result),
            Command::Admit(granted) => conn.admit(granted),
            Command::Refuse(refusal) => conn.refuse(refusal),
            Command::Close => {
                self.close();
                return;
            }
        }
        self.pair.settle();
    }

    /// Ends the session under both halves, as a transport that went away does.
    fn close(&mut self) {
        self.pair.dialer.closed(Closing::Done);
        self.pair.acceptor.closed(Closing::Done);
        self.pair.settle();
    }

    /// Settles each half's calls and hands it every other event.
    fn route(&mut self) {
        let dialer = mem::take(&mut self.pair.dialer_events);
        let acceptor = mem::take(&mut self.pair.acceptor_events);
        for (side, events) in [(Side::Dialer, dialer), (Side::Acceptor, acceptor)] {
            for event in events {
                match event {
                    Event::Answer { id, result } => {
                        if let Some(settle) = self.settles.remove(&(side, id)) {
                            settle(result);
                        }
                    }
                    other => {
                        let half = match side {
                            Side::Dialer => &mut self.dialer,
                            Side::Acceptor => &mut self.acceptor,
                        };
                        if matches!(other, Event::Closed(_)) {
                            half.closed = true;
                        }
                        // Nobody reads the events: the session still answers calls and settles its own.
                        half.events.send(other).ok();
                    }
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
