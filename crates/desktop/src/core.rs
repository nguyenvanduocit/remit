//! The client core as the desktop runs it (ADR-0128 decision 2): the `Replica` the web runs, stepped one input at a time,
//! what each step says carried out on a `Wire` and its one timer, and what the window is told handed back. The window
//! is the core's one tab. The same pump runs over QUIC on the shell thread (`quic.rs`) and over the fake server in
//! `--trace` (`fake.rs`), so a trace runs the core exactly as the app does.

use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

use link::{Millis, Transport};
use replica::{Input, Output, Replica, Told};

/// The window: the one tab of the core.
pub const TAB: replica::Tab = 1;

/// A transport the core dials and carries the link's streams on.
pub trait Wire: Transport {
    /// Opens a session; `Opened` or `DialFailed` follows, from `arrived` or from the shell.
    fn dial(&mut self);
    /// What the wire has for the core now: a fake server answers within the step that asked.
    fn arrived(&mut self) -> Vec<Input>;
}

pub struct Pump<W> {
    core: Replica,
    pub wire: W,
    /// When the core asked to be woken (`Input::Tick`).
    timer: Option<Millis>,
}

impl<W: Wire> Pump<W> {
    pub fn new(options: replica::Options, wire: W) -> Self {
        Self {
            core: Replica::new(options),
            wire,
            timer: None,
        }
    }

    /// Steps the core with `input` at `now`, then with each input that leads the wire to hand back; what the window
    /// is told, in the order the core said it.
    pub fn give(&mut self, now: Millis, input: Input) -> Vec<Told> {
        let mut queue = VecDeque::from([input]);
        let mut told = Vec::new();
        while let Some(input) = queue.pop_front() {
            for output in self.core.handle(now, input) {
                match output {
                    Output::Tell { tab, told: t } if tab == TAB => told.push(t),
                    // No other tab is served: the window asks as `TAB` alone.
                    Output::Tell { .. } => {}
                    Output::Dial => self.wire.dial(),
                    Output::Open(stream) => self.wire.open(stream),
                    Output::Send(stream, bytes) => self.wire.send(stream, bytes),
                    Output::Finish(stream) => self.wire.finish(stream),
                    Output::Reset(stream) => self.wire.reset(stream),
                    Output::Close(closing) => self.wire.close(closing),
                    Output::Timer(at) => self.timer = at,
                }
            }
            queue.extend(self.wire.arrived());
        }
        told
    }

    /// When the core wants `Input::Tick`; none: no timer.
    pub fn timer(&self) -> Option<Millis> {
        self.timer
    }
}

/// The wall clock in milliseconds, as the web's `Date.now()`: the core's times and the server's agree on it.
pub fn now() -> Millis {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// The precise clock the core times its decoding with, in milliseconds.
pub fn precise() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

/// How the window starts its core: who it is to the server, and the random text its request ids start with.
pub fn options(salt: String) -> replica::Options {
    replica::Options {
        node: link::Node {
            id: format!("desktop-{salt}"),
            public_key: String::new(),
        },
        bundle: None,
        salt,
        config: link::Config::default(),
        clock: precise,
    }
}

/// Random text for request ids: the time and the process, which no other run of this window shares.
pub fn salt() -> String {
    format!("{:x}{:x}", precise().to_bits(), std::process::id())
}
