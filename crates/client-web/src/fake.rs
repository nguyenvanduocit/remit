//! The server's half of a link session, in the module itself, for where there is no WebTransport (vitest in Node, the
//! verify pages): a real `link::Conn` accepting the client's, admitting it to the app contract, answering each call with
//! what the page's `answer` returns and pushing what the page hands `push`. Frames go between the two halves as bytes,
//! so the client runs exactly as it does over the wire.

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::Function;
use link::{
    Closing, Config, Conn, ErrorCode, Event, Failure, Node, Payload, Refusal, RefusalCode, StreamId, Transport,
};
use replica::Input;
use serde::Deserialize;
use serde_json::Value;
use wasm_bindgen::prelude::*;
use wire::app::{Call, HomeShown, Push};

use crate::shell::{Feed, Wire, now};

/// What the page's `answer` returns for a call: `{ ok: value }` or `{ err: { code, message } }`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Answered {
    Ok(Value),
    Err(Failure),
}

/// What the server does with the next session a client opens.
enum Door {
    /// Admits it to the app contract.
    Open,
    /// Turns its hello away, as a server does a revoked key or a page of an older build.
    Refuses(Refusal),
    /// Cannot be reached: the dial fails before any session opens.
    Down,
}

struct Server {
    conn: Conn,
    answer: Function,
    feed: Option<Feed>,
    door: Door,
    /// Which session `conn` is: a late answer of an earlier one is dropped, its call ids are not this one's.
    session: u64,
}

fn me() -> Node {
    Node {
        id: "fake-server".to_owned(),
        public_key: String::new(),
    }
}

/// The server a fake client talks to. `answer` is called with each call's body (`{ method, args }`) and returns how it
/// is answered.
#[derive(Clone)]
#[wasm_bindgen]
pub struct FakeServer {
    server: Rc<RefCell<Server>>,
}

#[wasm_bindgen]
impl FakeServer {
    #[wasm_bindgen(constructor)]
    pub fn new(answer: Function) -> FakeServer {
        Self {
            server: Rc::new(RefCell::new(Server {
                conn: Conn::accept(Config::default(), me()),
                answer,
                feed: None,
                door: Door::Open,
                session: 0,
            })),
        }
    }

    /// Pushes the JSON of a `Push` (`{ push, args }`) on the stream of `home` (none: the feed).
    pub fn push(&self, home: Option<String>, push: &str) -> Result<(), JsError> {
        let push: wire::app::Push =
            serde_json::from_str(push).map_err(|e| JsError::new(&format!("client-web: read a push: {e}")))?;
        self.server
            .borrow_mut()
            .conn
            .push(now(), home, &push)
            .map_err(|f| JsError::new(&f.to_string()))?;
        pump(&self.server);
        Ok(())
    }

    /// From now on the server turns each client away with `code` (`bad_credential`, `new_bundle`) and `reason`; the session
    /// up goes under, so the client's next dial is the one refused.
    pub fn refuse(&self, code: &str, reason: String) -> Result<(), JsError> {
        let code: RefusalCode = serde_json::from_value(Value::String(code.to_owned()))
            .map_err(|e| JsError::new(&format!("client-web: a refusal's code: {e}")))?;
        self.server.borrow_mut().door = Door::Refuses(Refusal { code, reason });
        self.lose();
        Ok(())
    }

    /// From now on the server cannot be reached: the session up goes under and every dial fails, until `come_up`.
    #[wasm_bindgen(js_name = goDown)]
    pub fn go_down(&self) {
        self.server.borrow_mut().door = Door::Down;
        self.lose();
    }

    /// The server admits clients again; the client's next dial opens a session.
    #[wasm_bindgen(js_name = comeUp)]
    pub fn come_up(&self) {
        self.server.borrow_mut().door = Door::Open;
    }

    /// The session goes under both halves, as a lost connection does; the client dials again.
    pub fn lose(&self) {
        self.server.borrow_mut().conn.closed(Closing::Done);
        pump(&self.server);
        let feed = self.server.borrow().feed.clone();
        if let Some(feed) = feed {
            feed.give(Input::Closed(Closing::Done));
        }
    }
}

impl FakeServer {
    pub(crate) fn wire(&self, feed: Feed) -> FakeWire {
        self.server.borrow_mut().feed = Some(feed);
        FakeWire {
            server: Rc::clone(&self.server),
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

fn failed(message: String) -> Failure {
    Failure::new(ErrorCode::Failed, message)
}

/// What the page's `answer` gave for a call: the answer now, or a promise of it.
enum Given {
    Now(Result<Payload, Failure>),
    Later(js_sys::Promise),
}

/// Calls the page's `answer` with the call as JSON (`{ method, args }`); a throw is a failed call, a thenable an answer to
/// wait for.
fn ask(answer: &Function, method: &str, args: &Payload) -> Given {
    let call = match link::decode_call::<wire::app::Call>(method, args) {
        Ok(call) => call,
        Err(failure) => return Given::Now(Err(failure)),
    };
    let Some(arg) = serde_json::to_string(&call)
        .ok()
        .and_then(|text| js_sys::JSON::parse(&text).ok())
    else {
        return Given::Now(Err(failed("the call does not encode".to_owned())));
    };
    match answer.call1(&JsValue::NULL, &arg) {
        Err(e) => Given::Now(Err(failed(format!("{e:?}")))),
        Ok(value) if is_thenable(&value) => Given::Later(js_sys::Promise::resolve(&value)),
        Ok(value) => Given::Now(read_answer(method, &value)),
    }
}

/// The server's half of `homes_shown`, as `rpc` applies it: each home named is pushed `home_shown` on its stream, from
/// which the client reads it. A session closing pushes nothing more.
fn mark_homes(conn: &mut Conn, method: &str, args: &Payload) {
    let Ok(Call::homes_shown { homes, seq }) = link::decode_call(method, args) else {
        return;
    };
    for home in homes {
        let mark = HomeShown {
            home: home.clone(),
            seq,
        };
        if conn.push(now(), Some(home), &Push::home_shown { mark }).is_err() {
            return;
        }
    }
}

fn is_thenable(value: &JsValue) -> bool {
    value.is_object() && js_sys::Reflect::get(value, &JsValue::from_str("then")).is_ok_and(|then| then.is_function())
}

/// The page's answer as a reply, typed as `method` answers; what is not an answer is a failed call.
fn read_answer(method: &str, value: &JsValue) -> Result<Payload, Failure> {
    let text = js_sys::JSON::stringify(value)
        .ok()
        .and_then(|s| s.as_string())
        .ok_or_else(|| failed("the answer is not JSON".to_owned()))?;
    match serde_json::from_str(&text).map_err(|e| failed(format!("the answer: {e}")))? {
        Answered::Ok(value) => link::encode_answer(wire::app::read_answer(method, value)),
        Answered::Err(failure) => Err(failure),
    }
}

/// Drives the server's conn until it has nothing left to do, handing the client what it sends. No borrow is held while
/// the page answers or the client takes bytes: either may come back here.
fn pump(server: &Rc<RefCell<Server>>) {
    loop {
        let mut sent = Sent::default();
        let (events, feed, answer) = {
            let mut s = server.borrow_mut();
            let events = s.conn.drive(&mut sent);
            (events, s.feed.clone(), s.answer.clone())
        };
        if events.is_empty() && sent.sends.is_empty() && sent.closes.is_empty() {
            return;
        }
        for event in events {
            match event {
                Event::Hello(_) => {
                    let mut s = server.borrow_mut();
                    match &s.door {
                        Door::Refuses(refusal) => {
                            let refusal = refusal.clone();
                            s.conn.refuse(refusal);
                        }
                        Door::Open | Door::Down => s.conn.admit(vec![wire::app::CAPABILITY.to_owned()]),
                    }
                }
                Event::Call { id, method, body, .. } => {
                    mark_homes(&mut server.borrow_mut().conn, &method, &body);
                    match ask(&answer, &method, &body) {
                        Given::Now(result) => server.borrow_mut().conn.reply(id, result),
                        Given::Later(promise) => {
                            // a slow server: the reply goes when the page's promise settles, a rejection failing the
                            // call
                            let (server, session) = (Rc::clone(server), server.borrow().session);
                            wasm_bindgen_futures::spawn_local(async move {
                                let result = match wasm_bindgen_futures::JsFuture::from(promise).await {
                                    Ok(value) => read_answer(&method, &value),
                                    Err(e) => Err(failed(format!("{e:?}"))),
                                };
                                if server.borrow().session != session {
                                    return;
                                }
                                server.borrow_mut().conn.reply(id, result);
                                pump(&server);
                            });
                        }
                    }
                }
                // The client offers nothing and pushes nothing; its closing ends the loop by itself.
                _ => {}
            }
        }
        for (stream, bytes) in sent.sends {
            server.borrow_mut().conn.written(stream, bytes.len());
            if let Some(feed) = &feed {
                feed.give(Input::Received { stream, bytes });
            }
        }
        for closing in sent.closes {
            if let Some(feed) = &feed {
                feed.give(Input::Closed(closing));
            }
        }
    }
}

pub(crate) struct FakeWire {
    server: Rc<RefCell<Server>>,
}

impl Transport for FakeWire {
    fn open(&mut self, _: StreamId) {}

    fn send(&mut self, stream: StreamId, bytes: Vec<u8>) {
        self.server.borrow_mut().conn.receive(stream, &bytes);
        pump(&self.server);
    }

    fn finish(&mut self, _: StreamId) {}

    fn reset(&mut self, _: StreamId) {}

    fn close(&mut self, closing: Closing) {
        self.server.borrow_mut().conn.closed(closing);
        pump(&self.server);
    }
}

impl Wire for FakeWire {
    /// A new session: a fresh server half, opened at once; none while the server is down, the dial failing as a browser's
    /// does when nothing answers.
    fn dial(&mut self) {
        let (feed, down) = {
            let mut s = self.server.borrow_mut();
            let down = matches!(s.door, Door::Down);
            if !down {
                s.conn = Conn::accept(Config::default(), me());
                s.session = s.session.wrapping_add(1);
            }
            (s.feed.clone(), down)
        };
        let Some(feed) = feed else { return };
        feed.give(if down {
            Input::DialFailed {
                message: "the server cannot be reached".to_owned(),
            }
        } else {
            Input::Opened {
                credential: "fake".to_owned(),
            }
        });
    }
}
