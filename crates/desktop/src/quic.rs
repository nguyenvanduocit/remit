//! The shell thread (ADR-0128 decision 2): a current-thread tokio runtime that owns the core's `Pump` over the server's
//! QUIC link, dials when the core asks, hands it what the session does, the window's asks and its timer, and sends what
//! the window is told through the window's mailbox, which wakes it.

use std::future::{Future, pending};
use std::pin::Pin;
use std::time::Duration;

use link::{Closing, Side, StreamId, Transport};
use link_quic::{Happened, Streams};
use replica::{Ask, Input, Told};
use tokio::sync::mpsc;

use crate::core::{Pump, TAB, Wire, now};
use crate::way_in::Credential;

/// The session the core's conn speaks on, once dialled.
#[derive(Default)]
struct Quic {
    streams: Option<Streams>,
    /// The core asked for a session: the loop starts a dial.
    dial: bool,
}

impl Transport for Quic {
    fn open(&mut self, stream: StreamId) {
        if let Some(s) = &mut self.streams {
            s.open(stream);
        }
    }

    fn send(&mut self, stream: StreamId, bytes: Vec<u8>) {
        if let Some(s) = &mut self.streams {
            s.send(stream, bytes);
        }
    }

    fn finish(&mut self, stream: StreamId) {
        if let Some(s) = &mut self.streams {
            s.finish(stream);
        }
    }

    fn reset(&mut self, stream: StreamId) {
        if let Some(s) = &mut self.streams {
            s.reset(stream);
        }
    }

    fn close(&mut self, closing: Closing) {
        if let Some(s) = &mut self.streams {
            s.close(closing);
        }
    }
}

impl Wire for Quic {
    /// The session before is let go (its tasks end), so nothing it reports reaches the conn of the next.
    fn dial(&mut self) {
        self.streams = None;
        self.dial = true;
    }

    fn arrived(&mut self) -> Vec<Input> {
        Vec::new()
    }
}

type Dialling = Pin<Box<dyn Future<Output = Result<Streams, String>>>>;

/// Starts the shell thread for `credential`; the window's asks go in through the returned sender, what it is told comes
/// out through `told`. The thread ends when the window does.
pub fn start(
    credential: Credential,
    options: replica::Options,
    told: gui::Sender<Vec<Told>>,
) -> std::io::Result<mpsc::UnboundedSender<Ask>> {
    let (asks, asked) = mpsc::unbounded_channel();
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    std::thread::Builder::new()
        .name("remit-core".to_owned())
        .spawn(move || runtime.block_on(drive(credential, options, asked, told)))?;
    Ok(asks)
}

async fn drive(
    credential: Credential,
    options: replica::Options,
    mut asked: mpsc::UnboundedReceiver<Ask>,
    told: gui::Sender<Vec<Told>>,
) {
    let mut pump = Pump::new(options, Quic::default());
    let mut dialling: Option<Dialling> = None;
    loop {
        if std::mem::take(&mut pump.wire.dial) {
            // A dial under way is given up for the new one: the core gave up on it.
            dialling = Some(Box::pin(open(credential.server.clone())));
        }
        let timer = pump.timer();
        let input = tokio::select! {
            ask = asked.recv() => match ask {
                Some(ask) => Input::Ask { tab: TAB, ask },
                // The window is gone.
                None => return,
            },
            opened = settle(&mut dialling) => {
                dialling = None;
                match opened {
                    Ok(streams) => {
                        pump.wire.streams = Some(streams);
                        Input::Opened { credential: credential.key.clone() }
                    }
                    Err(message) => Input::DialFailed { message },
                }
            }
            happened = next(&mut pump.wire.streams) => match happened {
                Happened::Read(stream, bytes) => Input::Received { stream, bytes: bytes.to_vec() },
                Happened::Written(stream, n) => Input::Written { stream, n },
                Happened::Closed(closing) => {
                    pump.wire.streams = None;
                    Input::Closed(closing)
                }
            },
            () = wake(timer) => Input::Tick,
        };
        let said = pump.give(now(), input);
        if !said.is_empty() && !told.send(said) {
            return;
        }
    }
}

/// A session on the link of the server at `server`, its streams ready for the core's conn.
async fn open(server: String) -> Result<Streams, String> {
    let link = remit_client::dial::link_json(&server).await?;
    let (url, trust) = remit_client::dial::trust_of(&link)?;
    let client = link_quic::client(trust).map_err(|e| e.to_string())?;
    let session = client.connect(url).await.map_err(|e| format!("dial the link: {e}"))?;
    Ok(Streams::start(session, Side::Acceptor))
}

async fn settle(dialling: &mut Option<Dialling>) -> Result<Streams, String> {
    match dialling {
        Some(dial) => dial.await,
        None => pending().await,
    }
}

async fn next(streams: &mut Option<Streams>) -> Happened {
    match streams {
        Some(s) => s.next().await,
        None => pending().await,
    }
}

async fn wake(at: Option<link::Millis>) {
    match at {
        Some(at) => tokio::time::sleep(Duration::from_millis(at.saturating_sub(now()))).await,
        None => pending().await,
    }
}
