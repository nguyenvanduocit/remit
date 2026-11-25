//! The link's transport in the browser: the browser's WebTransport through moq-dev's `web-transport`, as `link-quic`
//! is the native one. Each dial is one session; each stream has a task that writes what the conn hands it (saying how
//! many bytes went, so the conn can tell a peer that falls behind) and, when the stream has a receiving side, a task that
//! hands in every chunk read. What a task of an older session reports is dropped: the conn it was for is gone.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::future::poll_fn;
use std::pin::Pin;
use std::rc::Rc;

use futures_channel::mpsc;
use futures_core::Stream;
use js_sys::{Function, Promise};
use link::{Closing, Side, StreamId, StreamKind, Transport};
use replica::Input;
use serde::Deserialize;
use url::Url;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_transport::{ClientBuilder, RecvStream, SendStream, Session};

use crate::shell::{Feed, Wire};

/// Large enough for a frame's head and a good part of its body in one read.
const CHUNK: usize = 64 * 1024;

/// The stream reset code: the link's frames carry why; the code only says the stream was given up.
const RESET: u32 = 0;

/// Closes `session` with the code and reason that say why to the server.
fn close(session: &Session, closing: &Closing) {
    let (code, reason) = closing.code();
    session.close(code, reason);
}

/// What each dial asks the page: where the link is with the certificate the server presents, and the credential. Both
/// are asked again at every dial, as a server that restarted presents a new certificate.
pub(crate) struct Dialer {
    /// Called before each dial: where the link is now (`Link`, or a promise of it).
    link: Function,
    /// Called before each dial: the credential that session's hello carries (text, or a promise of it).
    credential: Function,
}

/// Where the server's link is and the certificate it presents, as the server's `/link.json` says it now.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Link {
    url: String,
    /// SHA-256 hashes of the server's certificate, hex, for a server whose certificate the browser does not trust by
    /// itself (the server's own, made new at each start); none: the system's roots.
    #[serde(default)]
    certificate_hashes: Vec<String>,
}

fn hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|at| {
            text.get(at..at.checked_add(2)?)
                .and_then(|b| u8::from_str_radix(b, 16).ok())
        })
        .collect()
}

impl Dialer {
    pub(crate) fn new(link: Function, credential: Function) -> Self {
        Self { link, credential }
    }

    pub(crate) fn wire(self, feed: Feed) -> Browser {
        Browser {
            dialer: Rc::new(self),
            feed,
            live: Rc::new(RefCell::new(Live::default())),
        }
    }
}

pub(crate) struct Browser {
    dialer: Rc<Dialer>,
    feed: Feed,
    live: Rc<RefCell<Live>>,
}

enum Write {
    Bytes(Vec<u8>),
    Finish,
    Reset,
}

/// A stream's writer, and whether the conn gave the stream up (its reader stops at its next chunk).
struct Writer {
    writes: mpsc::UnboundedSender<Write>,
    reset: Rc<Cell<bool>>,
}

#[derive(Default)]
struct Live {
    /// Counts dials: what a task of an older session reports is dropped.
    epoch: u64,
    session: Option<Session>,
    writers: HashMap<StreamId, Writer>,
    peer_bi: u64,
    peer_uni: u64,
}

/// One session's way back into the client.
#[derive(Clone)]
struct Tell {
    feed: Feed,
    live: Rc<RefCell<Live>>,
    epoch: u64,
}

/// A stream's ends as its tasks take them.
struct Ends {
    writes: mpsc::UnboundedReceiver<Write>,
    reset: Rc<Cell<bool>>,
}

impl Tell {
    fn current(&self) -> bool {
        self.live.borrow().epoch == self.epoch
    }

    fn give(&self, input: Input) {
        if self.current() {
            self.feed.give(input);
        }
    }

    /// Makes the writer of `stream` in this session; none once the session is not the current one.
    fn writer(&self, stream: StreamId) -> Option<Ends> {
        let mut live = self.live.borrow_mut();
        if live.epoch != self.epoch {
            return None;
        }
        let (tx, rx) = mpsc::unbounded();
        let reset = Rc::new(Cell::new(false));
        live.writers.insert(
            stream,
            Writer {
                writes: tx,
                reset: Rc::clone(&reset),
            },
        );
        Some(Ends { writes: rx, reset })
    }

    fn forget(&self, stream: StreamId) {
        let mut live = self.live.borrow_mut();
        if live.epoch == self.epoch {
            live.writers.remove(&stream);
        }
    }

    /// The server's streams are numbered here in the order they are accepted, per kind: the conn needs only that its
    /// ids are distinct and say who opened the stream and how.
    fn peer_stream(&self, kind: StreamKind) -> Option<StreamId> {
        let mut live = self.live.borrow_mut();
        if live.epoch != self.epoch {
            return None;
        }
        let counter = match kind {
            StreamKind::Bi => &mut live.peer_bi,
            StreamKind::Uni => &mut live.peer_uni,
        };
        let n = *counter;
        *counter = counter.wrapping_add(1);
        Some(StreamId {
            opener: Side::Acceptor,
            kind,
            n,
        })
    }
}

fn describe(value: &JsValue) -> String {
    value
        .dyn_ref::<js_sys::Error>()
        .map(|e| String::from(e.message()))
        .or_else(|| value.as_string())
        .unwrap_or_else(|| format!("{value:?}"))
}

/// What `page` answers when called: a value, or what the promise it returns settles to.
async fn ask(page: &Function) -> Result<JsValue, String> {
    let value = page.call0(&JsValue::NULL).map_err(|e| describe(&e))?;
    JsFuture::from(Promise::resolve(&value)).await.map_err(|e| describe(&e))
}

async fn credential_of(credential: &Function) -> Result<String, String> {
    ask(credential)
        .await?
        .as_string()
        .ok_or_else(|| "the credential is not text".to_owned())
}

/// The session to the link where it is now, trusting the certificate the server presents now.
async fn session_to(link: &Function) -> Result<Session, String> {
    let link: Link = serde_wasm_bindgen::from_value(ask(link).await?).map_err(|e| format!("where the link is: {e}"))?;
    let url = Url::parse(&link.url).map_err(|e| format!("the link's url {:?}: {e}", link.url))?;
    let hashes = link
        .certificate_hashes
        .iter()
        .map(|h| hex(h).ok_or_else(|| format!("{h:?} is not a hex hash")))
        .collect::<Result<Vec<_>, _>>()?;
    let builder = ClientBuilder::new();
    let client = if hashes.is_empty() {
        builder.with_system_roots()
    } else {
        builder.with_server_certificate_hashes(hashes)
    }
    .map_err(|e| e.to_string())?;
    client.connect(url).await.map_err(|e| e.to_string())
}

async fn connect(dialer: &Dialer) -> Result<(Session, String), String> {
    let credential = credential_of(&dialer.credential).await?;
    let session = session_to(&dialer.link).await?;
    Ok((session, credential))
}

async fn next(writes: &mut mpsc::UnboundedReceiver<Write>) -> Option<Write> {
    poll_fn(|cx| Pin::new(&mut *writes).poll_next(cx)).await
}

/// Writes what the conn hands the stream until it finishes, resets or the session goes. A stream with no sending side
/// (the server's one-way streams) only learns it was reset.
async fn write(stream: StreamId, mut send: Option<SendStream>, mut ends: Ends, tell: Tell) {
    while let Some(write) = next(&mut ends.writes).await {
        match write {
            Write::Bytes(bytes) => {
                let Some(out) = send.as_mut() else {
                    continue;
                };
                let n = bytes.len();
                if out.write(&bytes).await.is_err() {
                    break;
                }
                tell.give(Input::Written { stream, n });
            }
            Write::Finish => {
                // A finished stream still sends what was written; this side writes nothing more.
                if let Some(mut out) = send.take() {
                    out.finish().ok();
                }
            }
            Write::Reset => {
                ends.reset.set(true);
                if let Some(out) = send.as_mut() {
                    out.reset(RESET);
                }
                break;
            }
        }
    }
    tell.forget(stream);
}

/// Hands in every chunk the server sends on the stream until it finishes or resets it, or the conn gives it up.
async fn read(stream: StreamId, mut recv: RecvStream, reset: Rc<Cell<bool>>, tell: Tell) {
    while let Ok(Some(bytes)) = recv.read(CHUNK).await {
        if reset.get() {
            recv.stop(RESET);
            return;
        }
        tell.give(Input::Received {
            stream,
            bytes: bytes.to_vec(),
        });
    }
}

/// Starts the tasks of a stream of this session.
fn run(stream: StreamId, send: Option<SendStream>, recv: Option<RecvStream>, ends: Ends, tell: &Tell) {
    if let Some(recv) = recv {
        spawn_local(read(stream, recv, Rc::clone(&ends.reset), tell.clone()));
    }
    spawn_local(write(stream, send, ends, tell.clone()));
}

async fn accept_bi(session: Session, tell: Tell) {
    while let Ok((send, recv)) = session.accept_bi().await {
        let Some(stream) = tell.peer_stream(StreamKind::Bi) else {
            return;
        };
        let Some(ends) = tell.writer(stream) else {
            return;
        };
        run(stream, Some(send), Some(recv), ends, &tell);
    }
}

async fn accept_uni(session: Session, tell: Tell) {
    while let Ok(recv) = session.accept_uni().await {
        let Some(stream) = tell.peer_stream(StreamKind::Uni) else {
            return;
        };
        let Some(ends) = tell.writer(stream) else {
            return;
        };
        run(stream, None, Some(recv), ends, &tell);
    }
}

async fn watch_closed(session: Session, tell: Tell) {
    let closing = match session.closed().await {
        web_transport::Error::Session { code, reason } => Closing::from_code(code, &reason),
        _ => Closing::Done,
    };
    tell.give(Input::Closed(closing));
}

async fn dial(dialer: Rc<Dialer>, tell: Tell) {
    match connect(&dialer).await {
        Ok((session, credential)) => {
            {
                let mut live = tell.live.borrow_mut();
                if live.epoch != tell.epoch {
                    close(&session, &Closing::Done);
                    return;
                }
                live.session = Some(session.clone());
            }
            tell.give(Input::Opened { credential });
            spawn_local(accept_bi(session.clone(), tell.clone()));
            spawn_local(accept_uni(session.clone(), tell.clone()));
            spawn_local(watch_closed(session, tell));
        }
        Err(message) => tell.give(Input::DialFailed { message }),
    }
}

impl Browser {
    fn tell(&self) -> Tell {
        Tell {
            feed: self.feed.clone(),
            live: Rc::clone(&self.live),
            epoch: self.live.borrow().epoch,
        }
    }

    fn to_writer(&self, stream: StreamId, write: Write) {
        if let Some(writer) = self.live.borrow().writers.get(&stream) {
            // A stream whose task ended (the server reset it) takes nothing more.
            writer.writes.unbounded_send(write).ok();
        }
    }
}

impl Transport for Browser {
    fn open(&mut self, stream: StreamId) {
        let tell = self.tell();
        let Some(session) = self.live.borrow().session.clone() else {
            return;
        };
        let Some(ends) = tell.writer(stream) else {
            return;
        };
        spawn_local(async move {
            let opened = match stream.kind {
                StreamKind::Bi => session.open_bi().await.map(|(send, recv)| (send, Some(recv))),
                StreamKind::Uni => session.open_uni().await.map(|send| (send, None)),
            };
            // A stream that does not open means the session went: its closing reaches the core by its watcher.
            if let Ok((send, recv)) = opened {
                run(stream, Some(send), recv, ends, &tell);
            }
        });
    }

    fn send(&mut self, stream: StreamId, bytes: Vec<u8>) {
        self.to_writer(stream, Write::Bytes(bytes));
    }

    fn finish(&mut self, stream: StreamId) {
        self.to_writer(stream, Write::Finish);
    }

    fn reset(&mut self, stream: StreamId) {
        if let Some(writer) = self.live.borrow().writers.get(&stream) {
            writer.reset.set(true);
        }
        self.to_writer(stream, Write::Reset);
    }

    fn close(&mut self, closing: Closing) {
        if let Some(session) = self.live.borrow().session.as_ref() {
            close(session, &closing);
        }
    }
}

impl Wire for Browser {
    fn dial(&mut self) {
        let tell = {
            let mut live = self.live.borrow_mut();
            if let Some(old) = live.session.take() {
                close(&old, &Closing::Done);
            }
            *live = Live {
                epoch: live.epoch.wrapping_add(1),
                ..Live::default()
            };
            Tell {
                feed: self.feed.clone(),
                live: Rc::clone(&self.live),
                epoch: live.epoch,
            }
        };
        spawn_local(dial(Rc::clone(&self.dialer), tell));
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        if let Some(session) = self.live.borrow_mut().session.take() {
            close(&session, &Closing::Done);
        }
    }
}
