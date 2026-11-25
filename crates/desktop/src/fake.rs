//! The server's half of a link session in the window's own process, for `--trace` (ADR-0128 decision 6): a real
//! `link::Conn` accepting the core's, admitting it to the app contract, answering each call from a seed and pushing what
//! the trace hands it. Frames go between the two halves as bytes, so the core runs exactly as it does over QUIC; the web's
//! twin is `client-web/src/fake.rs`.

use link::{Closing, Config, Conn, ErrorCode, Event, Failure, Node, Payload, StreamId, Transport};
use replica::Input;
use serde_json::{Value, json};
use wire::app::{Call, HomeShown, Push};

use crate::core::{Wire, now};

/// What the seed answers a call with: the answer's JSON, or none for a call it does not answer.
pub type Answers = Box<dyn Fn(&Call) -> Option<Value>>;

pub struct Fake {
    conn: Conn,
    answers: Answers,
    arrived: Vec<Input>,
    /// Whether a session is open: a fake that went down fails every dial.
    up: bool,
}

fn me() -> Node {
    Node {
        id: "fake-server".to_owned(),
        public_key: String::new(),
    }
}

/// What the real server always answers, for a seed that leaves it out.
fn standing(call: &Call) -> Option<Value> {
    match call {
        Call::at_work {} => Some(json!({ "seats": [], "rev": 1 })),
        Call::homes_shown { .. } | Call::link_report { .. } => Some(json!({})),
        _ => None,
    }
}

impl Fake {
    pub fn new(answers: Answers) -> Self {
        Self {
            conn: Conn::accept(Config::default(), me()),
            answers,
            arrived: Vec::new(),
            up: true,
        }
    }

    /// The session goes under and every dial fails, until `come_up`: the server cannot be reached.
    pub fn go_down(&mut self) {
        self.up = false;
        self.conn.closed(Closing::Done);
        self.pump();
        self.arrived.push(Input::Closed(Closing::Done));
    }

    pub fn come_up(&mut self) {
        self.up = true;
    }

    fn answer(&self, method: &str, args: &Payload) -> Result<Payload, Failure> {
        let call: Call = link::decode_call(method, args)?;
        let value = (self.answers)(&call)
            .or_else(|| standing(&call))
            .ok_or_else(|| Failure::new(ErrorCode::Failed, format!("{method} is not answered in this trace")))?;
        link::encode_answer(wire::app::read_answer(method, value))
    }

    /// Drives the server's conn until it has nothing left to do, handing the core what it sends.
    fn pump(&mut self) {
        loop {
            let mut sent = Sent::default();
            let events = self.conn.drive(&mut sent);
            if events.is_empty() && sent.sends.is_empty() && sent.closes.is_empty() {
                return;
            }
            for event in events {
                match event {
                    Event::Hello(_) => self.conn.admit(vec![wire::app::CAPABILITY.to_owned()]),
                    Event::Call { id, method, body, .. } => {
                        self.mark_homes(&method, &body);
                        let result = self.answer(&method, &body);
                        self.conn.reply(id, result);
                    }
                    // The core offers nothing and pushes nothing; its closing ends the loop by itself.
                    _ => {}
                }
            }
            for (stream, bytes) in sent.sends {
                self.conn.written(stream, bytes.len());
                self.arrived.push(Input::Received { stream, bytes });
            }
            self.arrived.extend(sent.closes.into_iter().map(Input::Closed));
        }
    }

    /// The server's half of `homes_shown`, as `rpc` applies it: each home named is pushed `home_shown` on its stream.
    fn mark_homes(&mut self, method: &str, args: &Payload) {
        let Ok(Call::homes_shown { homes, seq }) = link::decode_call(method, args) else {
            return;
        };
        for home in homes {
            let mark = HomeShown {
                home: home.clone(),
                seq,
            };
            if self.conn.push(now(), Some(home), &Push::home_shown { mark }).is_err() {
                return;
            }
        }
    }
}

/// What the server's conn hands its transport.
#[derive(Default)]
struct Sent {
    sends: Vec<(StreamId, Vec<u8>)>,
    closes: Vec<Closing>,
}

impl Transport for Sent {
    fn open(&mut self, _: StreamId) {}
    fn send(&mut self, stream: StreamId, bytes: Vec<u8>) {
        self.sends.push((stream, bytes));
    }
    fn finish(&mut self, _: StreamId) {}
    fn reset(&mut self, _: StreamId) {}
    fn close(&mut self, closing: Closing) {
        self.closes.push(closing);
    }
}

impl Transport for Fake {
    fn open(&mut self, _: StreamId) {}

    fn send(&mut self, stream: StreamId, bytes: Vec<u8>) {
        self.conn.receive(stream, &bytes);
        self.pump();
    }

    fn finish(&mut self, _: StreamId) {}

    fn reset(&mut self, _: StreamId) {}

    fn close(&mut self, closing: Closing) {
        self.conn.closed(closing);
        self.pump();
    }
}

impl Wire for Fake {
    /// A new session with a fresh server half, opened at once; none while the server is down.
    fn dial(&mut self) {
        self.arrived.push(if self.up {
            self.conn = Conn::accept(Config::default(), me());
            Input::Opened {
                credential: "fake".to_owned(),
            }
        } else {
            Input::DialFailed {
                message: "the server cannot be reached".to_owned(),
            }
        });
    }

    fn arrived(&mut self) -> Vec<Input> {
        std::mem::take(&mut self.arrived)
    }
}
