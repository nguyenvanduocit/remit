//! The calls the core makes on its own (the snapshot, opening a chain, a stream's page, a draft's save) and the shell's,
//! each remembered by what it was for, so its answer goes to the rule that asked.

use std::collections::HashMap;

use link::{CallId, Conn, Contract, ErrorCode, Failure, Millis, Outgoing, Payload, Request};
use serde_json::Value;
use wire::app::{Call, FrameKind, decode_answer};

use crate::drafts::Saving;
use crate::io::Tab;
use crate::records::Asker;
use crate::shell::Key;
use crate::tally::Tally;

/// What a call was for.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Purpose {
    /// A tab's; its answer is that tab's, and a retry of it keeps `request`.
    Shell {
        asker: Asker,
        retry: Option<(String, String, Request)>,
    },
    Snapshot {
        attempt: u64,
    },
    /// The homes this session shows, as named the `seq`th time.
    HomesShown {
        seq: u64,
    },
    /// A chain's view, for every tab that opens it, as read the `seq`th time.
    ChainGet {
        chain: Key,
        seq: u64,
    },
    ChainEvents {
        chain: Key,
        seq: u64,
    },
    CardStream {
        tab: Tab,
        card: Key,
        generation: u64,
    },
    CardEarlier {
        tab: Tab,
        card: Key,
        generation: u64,
    },
    /// The seats at work, read by the sync of `attempt`.
    AtWork {
        attempt: u64,
    },
    AssistantStream {
        seq: u64,
    },
    EventGet {
        tab: Tab,
        seq: u64,
    },
    Archived {
        tab: Tab,
        generation: u64,
    },
    Repos {
        tab: Tab,
        /// Every repository's read (`Model::Repos`), else the open one's.
        every: bool,
        generation: u64,
    },
    DraftSave(Saving),
    Discard {
        id: Key,
    },
    /// The outbox's send of this row: its drafts leave the page once it went.
    Send {
        id: Key,
    },
    /// A minute's link report: nothing waits for its answer.
    LinkReport,
}

#[derive(Debug)]
pub(crate) struct Calls {
    pub(crate) conn: Option<Conn>,
    pub(crate) now: Millis,
    /// What each call in flight is for, and its method, which says its answer's type.
    purposes: HashMap<CallId, (Purpose, &'static str)>,
    /// Calls asked with no session: they fail at once, as the conn fails one asked while it is not live.
    unsent: Vec<(Purpose, Failure)>,
    salt: String,
    next_request: u64,
    /// What arrived on the link, counted for the server's telemetry.
    pub(crate) tally: Tally,
    /// The shell's precise clock, in milliseconds, which times decoding.
    clock: fn() -> f64,
}

impl Calls {
    pub(crate) fn new(salt: String, clock: fn() -> f64) -> Self {
        Self {
            conn: None,
            now: 0,
            purposes: HashMap::new(),
            unsent: Vec::new(),
            salt,
            next_request: 0,
            tally: Tally::default(),
            clock,
        }
    }

    /// Sends a call; its answer comes back to `purpose`. With no live session it fails here: a conn that ended fails a
    /// call among its own events, and those are dropped with it once its close is read, so a call asked while its close
    /// is being read would never be answered.
    pub(crate) fn ask<A>(&mut self, out: Outgoing<Call, A>, purpose: Purpose) {
        match self.conn.as_mut().filter(|conn| conn.is_live()) {
            Some(conn) => {
                let method = out.call().method();
                let pending = conn.call(self.now, out);
                self.purposes.insert(pending.id(), (purpose, method));
            }
            None => self.unsent.push((
                purpose,
                Failure::new(ErrorCode::Disconnected, "no session: nothing was sent"),
            )),
        }
    }

    /// What the call answered was for, its method, and its answer as the JSON the replica keeps; none for a call this
    /// core did not make.
    pub(crate) fn settled(
        &mut self,
        id: CallId,
        result: Result<Payload, Failure>,
    ) -> Option<(Purpose, &'static str, Result<Value, Failure>)> {
        let (purpose, method) = self.purposes.remove(&id)?;
        let result = result.and_then(|answer| {
            let (json, ms) = self.timed(|| as_json(method, &answer));
            // A report's own answer is not counted: it would start the next minute, and an idle page would report forever.
            if purpose != Purpose::LinkReport {
                self.tally
                    .frame(self.now, FrameKind::Answer, method, answer.as_bytes().len(), ms);
            }
            json
        });
        Some((purpose, method, result))
    }

    /// What `f` gives, and how long it took on the shell's precise clock.
    pub(crate) fn timed<T>(&self, f: impl FnOnce() -> T) -> (T, f64) {
        let start = (self.clock)();
        let out = f();
        (out, (self.clock)() - start)
    }

    /// Sends the minute's link report once it is due and a session is up to take it.
    pub(crate) fn report(&mut self) {
        if !self.live() {
            return;
        }
        if let Some(report) = self.tally.take(self.now, &self.salt) {
            self.ask(wire::app::client::link_report(report), Purpose::LinkReport);
        }
    }

    pub(crate) fn take_unsent(&mut self) -> Vec<(Purpose, Failure)> {
        std::mem::take(&mut self.unsent)
    }

    /// Whether a session is up: what the owner asks is sent, even while the latest data is still loading.
    pub(crate) fn live(&self) -> bool {
        self.conn.as_ref().is_some_and(Conn::is_live)
    }

    /// A new action's id: this page's salt (given by the shell, random) and a count, so ids never repeat.
    pub(crate) fn request(&mut self) -> Request {
        let id = format!("{}{:016x}", self.salt, self.next_request);
        self.next_request = self.next_request.wrapping_add(1);
        Request { id }
    }
}

/// An answer to a call of `method` as the JSON the replica keeps and the page reads, until the page draws models
/// (ADR-0095).
fn as_json(method: &str, answer: &Payload) -> Result<Value, Failure> {
    let answer = decode_answer(method, answer)?;
    serde_json::to_value(answer).map_err(|e| Failure::new(ErrorCode::InternalError, format!("the answer as JSON: {e}")))
}
