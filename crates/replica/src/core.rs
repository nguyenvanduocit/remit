//! The client core: one link session to the server and the replica it keeps in step. A snapshot on every (re)connect,
//! then each push applied to the record it names. What is pushed while the snapshot is on its way waits for it and is
//! applied over it, in the order it came; each record's revision says which copy is the newer.
//!
//! One core serves every tab of the browser (ADR-0095 decision 4): the session, the catalog, the Assistant, the
//! owner's typed words and his sends are shared; what a tab opened (a chain, an event, the archived pages), the models it
//! shows and whether the owner sees it are its own. The session stays while any tab is served and closes with the last.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashMap};

use link::{
    CallKind, Closing, Config, Conn, ErrorCode, Event, Failure, Hello, Millis, Node, Outgoing, RefusalCode, Request,
    StreamId, Transport,
};
use serde_json::Value;
use wire::app::{Call, FaultKind, FrameKind, Push, RepairReason, Snapshot, Who, client};

use crate::archived::Archived;
use crate::assistant::Assistant;
use crate::banner;
use crate::calls::{Calls, Purpose};
use crate::catalog::Catalog;
use crate::drafts::{Drafts, Homes, Ready, Saving};
use crate::event::OpenEvent;
use crate::fresh::{self, Fresh, Reads};
use crate::held::{HeldChains, Open, Read};
use crate::io::{Input, Output, Tab};
use crate::models::Models;
use crate::opened::{OpenChain, Opened};
use crate::outbox::{Asked, Outbox};
use crate::records::{Asker, Out, push_arg, read, take};
use crate::repos::Repos;
use crate::shell::{Ask, Change, Key, Model, Notice, Phase, Retry, State, Told, What};

/// Said when an action is not asked because there is no session: sent later, it would act when the owner no longer
/// expects it.
pub(crate) const OFFLINE: &str = "Not connected to the server: nothing was done. Try again once it is back.";

/// How long loading what the app shows may take before it counts as failed and is tried again.
const SNAPSHOT_TIMEOUT: Millis = 20_000;

/// How long a dial may take before it counts as failed and is tried again. A browser can leave one unsettled for good
/// (a page that entered the back/forward cache): without an end, no later dial would go.
const DIAL_TIMEOUT: Millis = 10_000;

/// What the core needs from its shell once: who this client is, the build of its contracts (none for a development
/// build), randomness for its request ids, and the link's timeouts.
#[derive(Debug, Clone)]
pub struct Options {
    pub node: Node,
    pub bundle: Option<String>,
    /// Random text, made fresh for each page or run: request ids are this and a count.
    pub salt: String,
    pub config: Config,
    /// A precise clock in milliseconds (the page's `performance.now()`), which times decoding for the link's telemetry.
    pub clock: fn() -> f64,
}

/// How long to wait before try number `attempt` (from 0): a second, doubling, at most five.
fn retry_delay(attempt: u32) -> Millis {
    1_000_u64.checked_shl(attempt).map_or(5_000, |ms| ms.min(5_000))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Idle,
    Snapshot { deadline: Millis },
    Reloads { deadline: Millis, left: u32 },
    Waiting { until: Millis },
}

/// Loading what the app shows, until it works.
#[derive(Debug)]
struct Sync {
    /// Counts connections and tries: what answers an older one is dropped.
    attempt: u64,
    tries: u32,
    stage: Stage,
    /// Pushes that came while the snapshot was on its way.
    early: Option<Vec<(Push, Value)>>,
    /// The sync attempt whose reconnect asked for the Assistant's stream again.
    assistant: Option<u64>,
}

#[derive(Debug)]
pub struct Replica {
    opts: Options,
    calls: Calls,
    phase: Phase,
    why: String,
    refused: Option<RefusalCode>,
    wanted: bool,
    /// Until when the dial under way may take, if one is.
    dialling: Option<Millis>,
    redial_at: Option<Millis>,
    redials: u32,
    /// When the link went, if the page had something to keep showing, until it is live again.
    lost_at: Option<Millis>,
    /// Until when the page says the link is back.
    back_until: Option<Millis>,
    sync: Sync,
    shown: Shown,
    catalog: Catalog,
    assistant: Assistant,
    /// The chains the tabs show or open, each held once.
    held: HeldChains,
    /// The tabs served, each with what it opened.
    tabs: BTreeMap<Tab, Page>,
    drafts: Drafts,
    outbox: Outbox,
    /// The records, by model, and the models the shell shows.
    models: Models,
    fresh: Fresh,
    /// A place's action whose answer never came, by tab and place: asking the same key again retries it with its
    /// request.
    unsettled: HashMap<(Tab, String), (String, Request)>,
    /// The events pushed this step, by key, with when they happened (the server's clock, in milliseconds): the delay
    /// of each ends when a tab is handed it (decision 12).
    pushed_at: HashMap<Key, i64>,
    out: Out,
    outputs: Vec<Output>,
    last_timer: Option<Option<Millis>>,
}

/// One tab's own: what it opened, whether the owner sees it, and the state it was last told.
#[derive(Debug)]
struct Page {
    opened: OpenChain,
    event: OpenEvent,
    archived: Archived,
    /// The repository open, and every repository.
    repo: Repos,
    repos: Repos,
    /// The owner does not see it (`document.hidden`).
    hidden: bool,
    last_state: Option<State>,
}

impl Page {
    fn new(tab: Tab) -> Self {
        Self {
            opened: OpenChain::new(tab),
            event: OpenEvent::new(tab),
            archived: Archived::new(tab),
            repo: Repos::one(tab),
            repos: Repos::every(tab),
            hidden: false,
            last_state: None,
        }
    }
}

/// The homes this session told the server it shows (`homes_shown`, ADR-0095): none told yet in this session, the last
/// telling's seq, and the homes it named anew whose `home_shown` of that telling has not come yet. Such a home is read
/// once its mark comes, and what streams there before it is dropped: it may have been sent before the server learned
/// that this session let the home go, and would land on the read.
#[derive(Debug, Default)]
struct Shown {
    told: Option<Vec<Key>>,
    seq: u64,
    marks: HashMap<Key, u64>,
}

/// The conn's `Transport`: what it asks goes out as outputs, in order.
struct Collect<'a>(&'a mut Vec<Output>);

impl Transport for Collect<'_> {
    fn open(&mut self, stream: StreamId) {
        self.0.push(Output::Open(stream));
    }

    fn send(&mut self, stream: StreamId, bytes: Vec<u8>) {
        self.0.push(Output::Send(stream, bytes));
    }

    fn finish(&mut self, stream: StreamId) {
        self.0.push(Output::Finish(stream));
    }

    fn reset(&mut self, stream: StreamId) {
        self.0.push(Output::Reset(stream));
    }

    fn close(&mut self, closing: Closing) {
        self.0.push(Output::Close(closing));
    }
}

/// A call of the contract as the shell wrote it, with the request id the core gives a changing one.
fn call_of(mut call: Value, request: Option<&Request>) -> Result<Call, Failure> {
    if let Some(request) = request {
        let fields = call
            .as_object_mut()
            .ok_or_else(|| Failure::new(ErrorCode::InvalidArgument, "a call is an object"))?;
        let args = fields
            .entry("args")
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        let args = args
            .as_object_mut()
            .ok_or_else(|| Failure::new(ErrorCode::InvalidArgument, "a call's args are an object"))?;
        args.insert("request".to_owned(), serde_json::json!({ "id": request.id }));
    }
    read(&call)
}

/// The kind of the contract's call that `call` names.
fn kind_of(call: &Value) -> Result<CallKind, Failure> {
    let method = call.get("method").and_then(Value::as_str).unwrap_or_default();
    wire::app::CALLS
        .iter()
        .find(|spec| spec.name == method)
        .map(|spec| spec.kind)
        .ok_or_else(|| Failure::new(ErrorCode::InvalidArgument, format!("no call is named {method:?}")))
}

/// The answer never came: whether the server acted is not known, so asking again retries with the same request.
fn unanswered(result: &Result<Value, Failure>) -> bool {
    matches!(result, Err(f) if matches!(f.code, ErrorCode::Timeout | ErrorCode::Disconnected))
}

fn why_closed(closing: &Closing) -> String {
    match closing {
        Closing::Done => "the connection was lost".to_owned(),
        Closing::Behind => "this page fell behind the server".to_owned(),
        Closing::Broken(why) => why.clone(),
        Closing::Refused(r) => r.reason.clone(),
    }
}

impl Replica {
    pub fn new(opts: Options) -> Self {
        let calls = Calls::new(opts.salt.clone(), opts.clock);
        Self {
            opts,
            calls,
            phase: Phase::Offline,
            why: String::new(),
            refused: None,
            wanted: false,
            dialling: None,
            redial_at: None,
            redials: 0,
            lost_at: None,
            back_until: None,
            sync: Sync {
                attempt: 0,
                tries: 0,
                stage: Stage::Idle,
                early: None,
                assistant: None,
            },
            shown: Shown::default(),
            catalog: Catalog::default(),
            assistant: Assistant::default(),
            held: HeldChains::default(),
            tabs: BTreeMap::new(),
            drafts: Drafts::default(),
            outbox: Outbox::default(),
            models: Models::default(),
            fresh: Fresh::default(),
            unsettled: HashMap::new(),
            pushed_at: HashMap::new(),
            out: Out::default(),
            outputs: Vec::new(),
            last_timer: None,
        }
    }

    /// Takes one input at `now` (the shell's clock, in milliseconds) and says what to do.
    pub fn handle(&mut self, now: Millis, input: Input) -> Vec<Output> {
        self.calls.now = now;
        self.input(input);
        self.settle();
        self.flush()
    }

    fn input(&mut self, input: Input) {
        match input {
            Input::Ask { tab, ask } => self.ask(tab, ask),
            Input::Left { tab } => self.leave(tab),
            Input::Opened { credential } => {
                self.dialling = None;
                if !self.wanted {
                    // Every tab left while it was dialled: the session it opened is let go at once.
                    self.outputs.push(Output::Close(Closing::Done));
                    return;
                }
                let hello = Hello {
                    node: self.opts.node.clone(),
                    credential,
                    capabilities: vec![wire::app::CAPABILITY.to_owned()],
                    offers: Vec::new(),
                    bundle: self.opts.bundle.clone(),
                };
                self.calls.conn = Some(Conn::dial(self.opts.config, hello));
                // A new session is pushed every home until it is told which it shows, and was told none: it names them
                // all anew and reads each from its mark. Its seq goes on, as the server applies no telling older than
                // the last it applied.
                self.shown = Shown {
                    seq: self.shown.seq,
                    ..Shown::default()
                };
            }
            Input::DialFailed { message } => self.dial_failed(message),
            Input::Received { stream, bytes } => {
                if let Some(conn) = self.calls.conn.as_mut() {
                    conn.receive(stream, &bytes);
                }
            }
            Input::Written { stream, n } => {
                if let Some(conn) = self.calls.conn.as_mut() {
                    conn.written(stream, n);
                }
            }
            Input::Closed(closing) => {
                if let Some(conn) = self.calls.conn.as_mut() {
                    conn.closed(closing);
                }
            }
            Input::Tick => self.tick(),
        }
    }

    /// What tab `tab` asks; a tab first heard of is served from now on, and told what to keep with the rest.
    fn ask(&mut self, tab: Tab, ask: Ask) {
        let page = match self.tabs.entry(tab) {
            Entry::Occupied(page) => page.into_mut(),
            Entry::Vacant(new) => {
                self.outbox.keep_again();
                new.insert(Page::new(tab))
            }
        };
        match ask {
            Ask::Show { model } => self.models.show(tab, model),
            Ask::Hide { model } => self.models.hide(tab, &model),
            Ask::Hidden { hidden } => page.hidden = hidden,
            Ask::Fault { kind, message, at } => {
                let chain = page.opened.shown().unwrap_or_default().to_owned();
                self.calls.tally.fault(self.calls.now, kind, &message, &at, &chain);
            }
            Ask::Connect => {
                self.refused = None;
                self.wanted = true;
                self.redial_at = None;
                self.dial();
            }
            Ask::Call { ticket, retry, call } => self.shell_call(Asker { tab, ticket }, retry, call),
            Ask::OpenChain { id } => self.open_chain(tab, id, None),
            Ask::LoadStream { card } => {
                let truncated = self.held.truncated(page.opened.shown());
                page.opened.load_stream(&mut self.calls, card, truncated);
            }
            Ask::LoadEarlier { card } => page.opened.load_earlier(&mut self.calls, card),
            Ask::LoadAssistant => {
                self.assistant.load();
            }
            Ask::OpenEvent { id } => page.event.open(&mut self.calls, id, None),
            Ask::CloseEvent => page.event.close(&mut self.out),
            Ask::ReadArchived => page.archived.read_next(&mut self.calls),
            Ask::OpenRepo { repo } => page.repo.open_one(&mut self.calls, repo, &mut self.out),
            Ask::OpenRepos { open } => page.repos.open_every(&mut self.calls, open, &mut self.out),
            Ask::Write { draft } => {
                let homes = Homes {
                    chains: &mut self.held,
                    assistant: &mut self.assistant,
                };
                if let Err(failure) = self.drafts.write(&mut self.calls, &homes, draft, &mut self.out) {
                    self.out.notice(Notice::CouldNot {
                        what: What::SaveDraft,
                        failure,
                    });
                }
            }
            Ask::Discard { id } => {
                let mut homes = Homes {
                    chains: &mut self.held,
                    assistant: &mut self.assistant,
                };
                self.drafts.discard(&mut self.calls, &mut homes, &id, &mut self.out);
            }
            Ask::Send {
                ticket,
                ids,
                call,
                at,
                text,
            } => {
                let asked = Asked {
                    asker: Asker { tab, ticket },
                    ids,
                    call,
                    at,
                    text,
                };
                self.outbox.add(&mut self.calls, asked, &mut self.out);
            }
            Ask::Restore { kept } => self
                .outbox
                .restore(kept, self.calls.now, &mut self.drafts, &mut self.out),
            Ask::Resend { id } => self.outbox.resend(&mut self.calls, &id, &mut self.out),
            Ask::Unsend { id } => {
                if let Some(Some(asker)) = self.outbox.unsend(&id, &mut self.out) {
                    let failure = Failure::new(ErrorCode::Failed, "taken back to edit: nothing was sent");
                    self.answer(asker, Err(failure));
                }
            }
        }
    }

    /// The tab is gone: what it opened and showed goes, a reload a sync waited for in it counts as done, and with the
    /// last tab the session closes.
    fn leave(&mut self, tab: Tab) {
        let Some(page) = self.tabs.remove(&tab) else {
            return;
        };
        self.models.leave(tab);
        self.unsettled.retain(|(of, _), _| *of != tab);
        for attempt in [page.opened.by_sync(), page.event.by_sync()].into_iter().flatten() {
            self.reloaded(attempt, Ok(()));
        }
        if !self.tabs.is_empty() {
            return;
        }
        self.wanted = false;
        self.redial_at = None;
        if self.calls.conn.is_some() {
            self.outputs.push(Output::Close(Closing::Done));
            if let Some(conn) = self.calls.conn.as_mut() {
                conn.closed(Closing::Done);
            }
        }
    }
    fn dial(&mut self) {
        if self.wanted && self.calls.conn.is_none() && self.dialling.is_none() && self.refused.is_none() {
            self.dialling = Some(self.calls.now.saturating_add(DIAL_TIMEOUT));
            self.outputs.push(Output::Dial);
        }
    }

    /// The dial under way did not open a session: the next goes after a wait. A shell dialling again drops what the
    /// one given up on reports (browser.rs `Live::epoch`).
    fn dial_failed(&mut self, message: String) {
        self.dialling = None;
        self.why = message;
        self.redial_later();
    }

    fn redial_later(&mut self) {
        if self.refused.is_some() || !self.wanted {
            return;
        }
        self.redial_at = Some(self.calls.now.saturating_add(retry_delay(self.redials)));
        self.redials = self.redials.saturating_add(1);
    }

    fn tick(&mut self) {
        let now = self.calls.now;
        if let Some(conn) = self.calls.conn.as_mut() {
            conn.tick(now);
        }
        if self.dialling.is_some_and(|until| until <= now) {
            self.dial_failed("the server did not answer in time".to_owned());
        }
        if self.redial_at.is_some_and(|at| at <= now) {
            self.redial_at = None;
            self.dial();
        }
        match self.sync.stage {
            Stage::Snapshot { deadline } | Stage::Reloads { deadline, .. } if deadline <= now => {
                self.flush_early();
                self.load_failed("the latest data did not load in time".to_owned());
            }
            Stage::Waiting { until } if until <= now => self.begin_load(),
            _ => {}
        }
        self.drafts.tick(&mut self.calls, self.outbox.holds(), &mut self.out);
        self.outbox.tick(now, &mut self.out);
        self.calls.report();
    }

    /// Drives the conn and acts on what happened until nothing is left to do.
    fn settle(&mut self) {
        loop {
            self.tend();
            self.show();
            let events = match self.calls.conn.as_mut() {
                Some(conn) => conn.drive(&mut Collect(&mut self.outputs)),
                None => Vec::new(),
            };
            let unsent = self.calls.take_unsent();
            let ready = std::mem::take(&mut self.drafts.ready);
            let started = if self.calls.live() {
                self.outbox.start(self.calls.now, &mut self.out)
            } else {
                Vec::new()
            };
            if events.is_empty() && unsent.is_empty() && ready.is_empty() && started.is_empty() {
                return;
            }
            for (id, ids) in started {
                self.drafts.send(&mut self.calls, id, &ids, &mut self.out);
            }
            for event in events {
                self.on_event(event);
            }
            for (purpose, failure) in unsent {
                self.on_answer(purpose, Err(failure));
            }
            for send in ready {
                self.send_ready(send);
            }
        }
    }

    fn on_event(&mut self, event: Event) {
        match event {
            Event::Welcomed(_) => {
                self.phase = Phase::Syncing;
                self.why.clear();
                self.redials = 0;
                self.start_sync();
            }
            Event::Refused(refusal) => {
                self.refused = Some(refusal.code);
                self.why = refusal.reason;
            }
            Event::Answer { id, result } => {
                if let Some((purpose, method, result)) = self.calls.settled(id, result) {
                    if let Err(f) = &result {
                        self.call_failed(&purpose, method, f);
                    }
                    self.on_answer(purpose, result);
                }
            }
            Event::Push { contract, name, body } if contract == wire::app::CAPABILITY => {
                // A push this build does not know: the server serves another build, which the hello's bundle refuses.
                let (push, ms) = self.calls.timed(|| link::decode_push::<Push>(&name, &body));
                let now = self.calls.now;
                self.calls
                    .tally
                    .frame(now, FrameKind::Push, &name, body.as_bytes().len(), ms);
                match push {
                    Ok(push) => self.on_push(push),
                    Err(_) => self.calls.tally.repair(now, RepairReason::Decode, ""),
                }
            }
            Event::Closed(closing) => {
                self.calls.conn = None;
                if let Closing::Refused(r) = &closing {
                    self.refused = Some(r.code);
                }
                self.phase = Phase::Offline;
                self.why = why_closed(&closing);
                if self.catalog.loaded {
                    self.lost_at.get_or_insert(self.calls.now);
                }
                self.sync.attempt = self.sync.attempt.wrapping_add(1);
                self.sync.stage = Stage::Idle;
                self.sync.early = None;
                self.redial_later();
            }
            // This client offers no contract, so the conn answers any call itself; `behind` is followed by the close.
            Event::Hello(_) | Event::Call { .. } | Event::Push { .. } | Event::Behind => {}
        }
    }

    /// Tells the server the homes this session shows when they changed since it was last told, and reads those waiting
    /// to be read once they can be (`read_shown`): a home is named before it is read, so nothing pushed in between is
    /// missed, and one named anew is read from its `home_shown`, so nothing pushed before lands on the read. With no
    /// session nothing is named or read: what is being opened waits for the next, which names it first. Dropped meanwhile,
    /// it would be named only once the snapshot opened it again, after a session that is pushed every home until it names
    /// its own had let go of it, and a push sent before could land on that read.
    fn show(&mut self) {
        if !self.calls.live() {
            return;
        }
        let mut homes: Vec<Key> = self
            .tabs
            .values()
            .map(|p| p.opened.home())
            .chain([self.assistant.home()])
            .flatten()
            .filter(|h| !h.is_empty())
            .map(str::to_owned)
            .collect();
        homes.sort();
        homes.dedup();
        if self.shown.told.as_ref() != Some(&homes) {
            let seq = self.shown.seq.wrapping_add(1);
            let told = self.shown.told.take().unwrap_or_default();
            self.shown.marks.retain(|home, _| homes.contains(home));
            for home in homes.iter().filter(|h| !told.contains(h)) {
                self.shown.marks.insert(home.clone(), seq);
            }
            self.shown.seq = seq;
            self.shown.told = Some(homes.clone());
            self.calls
                .ask(client::homes_shown(homes, seq), Purpose::HomesShown { seq });
        }
        self.read_shown();
    }

    /// Reads what waits to be read in the homes told, but those whose `home_shown` has not come yet.
    fn read_shown(&mut self) {
        let marks = &self.shown.marks;
        self.held.read(&mut self.calls, |home| !marks.contains_key(home));
        if self.assistant.home().is_some_and(|home| !marks.contains_key(home)) {
            self.assistant.read(&mut self.calls);
        }
    }

    /// The server applied the telling of seq, which named mark's home: a home waiting for it is read from here.
    fn home_shown(&mut self, home: &str, seq: u64) {
        if self.shown.marks.get(home).is_some_and(|wanted| *wanted <= seq) {
            self.shown.marks.remove(home);
            self.read_shown();
        }
    }

    /// Keeps each chain held while a tab shows or opens it, and has each tab read the models of the chain it shows.
    fn tend(&mut self) {
        let shown: Vec<&str> = self.tabs.values().filter_map(|p| p.opened.shown()).collect();
        let opening: Vec<&str> = self
            .tabs
            .values()
            .map(|p| p.opened.opening.as_str())
            .filter(|o| !o.is_empty())
            .collect();
        let homes: Vec<&str> = self.tabs.values().filter_map(|p| p.opened.home()).collect();
        self.held.tend(&shown, &opening, &homes, &mut self.out);
        for (tab, page) in &self.tabs {
            self.models.show_chain(*tab, page.opened.shown());
        }
    }

    /// Tab `tab` opens chain id: it shows what is held of it at once when that is live, else waits for its read.
    fn open_chain(&mut self, tab: Tab, id: Key, by_sync: Option<u64>) {
        let Some(page) = self.tabs.get_mut(&tab) else { return };
        page.opened.open(id.clone(), by_sync);
        if let Open::Held = self.held.open(&id) {
            let read = Read {
                events: self.held.events(&id, &self.models),
                early: Vec::new(),
            };
            self.chain_read(tab, Ok(&read));
        }
    }

    /// The read of the chain tab `tab` opens ended: it shows the chain, its windows started on the events read and
    /// what was pushed meanwhile, or is told it could not.
    fn chain_read(&mut self, tab: Tab, read: Result<&Read, &Failure>) {
        let Some(page) = self.tabs.get_mut(&tab) else { return };
        let opened = match read {
            Ok(read) => {
                let by_sync = page.opened.opened(read.events.clone(), &mut self.out);
                for (push, record) in &read.early {
                    page.opened.on_push(push, record, &mut self.out);
                }
                Opened {
                    by_sync,
                    result: Ok(()),
                }
            }
            Err(failure) => page.opened.failed(failure.clone(), &mut self.out),
        };
        self.chain_opened(tab, opened);
    }

    fn start_sync(&mut self) {
        self.sync.tries = 0;
        self.begin_load();
    }

    fn begin_load(&mut self) {
        self.sync.attempt = self.sync.attempt.wrapping_add(1);
        self.sync.stage = Stage::Snapshot {
            deadline: self.calls.now.saturating_add(SNAPSHOT_TIMEOUT),
        };
        self.sync.early = Some(Vec::new());
        let purpose = Purpose::Snapshot {
            attempt: self.sync.attempt,
        };
        self.calls.ask(client::app_snapshot(), purpose);
    }

    fn loaded(&mut self) {
        self.sync.stage = Stage::Idle;
        if matches!(self.phase, Phase::Syncing | Phase::Stale) {
            if self.phase == Phase::Stale || self.lost_at.is_some() {
                self.back_until = Some(self.calls.now.saturating_add(banner::BACK_SHOWN));
            }
            self.phase = Phase::Live;
            self.why.clear();
            self.lost_at = None;
        }
    }

    fn load_failed(&mut self, why: String) {
        self.sync.attempt = self.sync.attempt.wrapping_add(1);
        if matches!(self.phase, Phase::Syncing | Phase::Stale) {
            self.phase = Phase::Stale;
            self.why = why;
        }
        self.sync.stage = Stage::Waiting {
            until: self.calls.now.saturating_add(retry_delay(self.sync.tries)),
        };
        self.sync.tries = self.sync.tries.saturating_add(1);
    }

    /// What was pushed while the snapshot was on its way is applied over it, in the order it came.
    fn flush_early(&mut self) {
        for (push, record) in self.sync.early.take().unwrap_or_default() {
            self.apply_push(&push, &record);
        }
    }

    fn snapshot_answered(&mut self, result: Result<Value, Failure>) {
        let snap = result.and_then(|json| Ok((read::<Snapshot>(&json)?, json)));
        let (snap, mut json) = match snap {
            Ok(snap) => snap,
            Err(failure) => {
                self.flush_early();
                self.load_failed(failure.message);
                return;
            }
        };
        match &snap.assistant {
            Some(view) => self.assistant.set(view, take(&mut json, "assistant"), &mut self.out),
            None => self.assistant.forget(&mut self.out),
        }
        self.assistant.set_feed(take(&mut json, "assistantFeed"), &mut self.out);
        let owner = matches!(snap.who, Who::Owner);
        self.catalog.apply_snapshot(snap, json, &mut self.out);
        for page in self.tabs.values_mut() {
            page.archived.forget(&mut self.out);
            page.repo.read_again(&mut self.calls, &mut self.out);
            page.repos.read_again(&mut self.calls, &mut self.out);
        }
        self.flush_early();
        let attempt = self.sync.attempt;
        let mut left = 0_u32;
        let mut reopen = Vec::new();
        for (tab, page) in &mut self.tabs {
            if let Some(id) = page
                .opened
                .reload(&self.catalog, |c| self.held.archived(c), &mut self.out)
            {
                reopen.push((*tab, id));
                left = left.saturating_add(1);
            }
            if page.event.reload(&mut self.calls, attempt) {
                left = left.saturating_add(1);
            }
        }
        // Nothing held is live after a reconnect: each chain is read again, once for every tab that shows it.
        self.held.stale();
        for (tab, id) in reopen {
            self.open_chain(tab, id, Some(attempt));
        }
        if self.assistant.reload() {
            self.sync.assistant = Some(attempt);
            left = left.saturating_add(1);
        }
        // The seats at work are read, not in the snapshot; only the owner's page reads them.
        if owner {
            self.calls.ask(client::at_work(), Purpose::AtWork { attempt });
            left = left.saturating_add(1);
        } else {
            self.catalog.forget_at_work(&mut self.out);
        }
        if left == 0 {
            self.loaded();
        } else {
            self.sync.stage = Stage::Reloads {
                deadline: self.calls.now.saturating_add(SNAPSHOT_TIMEOUT),
                left,
            };
        }
    }

    /// A reload a sync waited for ended.
    fn reloaded(&mut self, attempt: u64, result: Result<(), Failure>) {
        if attempt != self.sync.attempt {
            return;
        }
        let Stage::Reloads { deadline, left } = self.sync.stage else {
            return;
        };
        match result {
            Err(failure) => self.load_failed(failure.message),
            Ok(()) => match left.checked_sub(1) {
                Some(0) | None => self.loaded(),
                Some(left) => self.sync.stage = Stage::Reloads { deadline, left },
            },
        }
    }

    fn on_push(&mut self, push: Push) {
        // Acted on as it comes, held or not while a snapshot loads: what came on the home's stream before it is dropped
        // as it came, and what comes after waits as everything else does.
        if let Push::home_shown { mark } = &push {
            return self.home_shown(&mark.home, mark.seq);
        }
        if push
            .streamed_home()
            .is_some_and(|home| self.shown.marks.contains_key(home))
        {
            return;
        }
        if let Push::event_added { event } = &push {
            let at_ns = event.event.created_at.0.unix_timestamp_nanos();
            let at_ms = i64::try_from(at_ns / 1_000_000).unwrap_or(i64::MAX);
            self.pushed_at.insert(crate::events::key(event.event.id), at_ms);
        }
        // The record the page is told of, as JSON, until the page draws models (ADR-0095). Every pushed type writes
        // JSON (each has a string key), so none is dropped here.
        let Ok(record) = serde_json::to_value(&push).map(push_arg) else {
            return;
        };
        match self.sync.early.as_mut() {
            Some(early) => early.push((push, record)),
            None => self.apply_push(&push, &record),
        }
    }

    fn apply_push(&mut self, push: &Push, record: &Value) {
        // Said once, however many tabs show the chain whose agent finished.
        if let Some(notice) = self.held.turn_finished(push) {
            self.out.notice(notice);
        }
        self.catalog.on_push(push, record, &mut self.out);
        match push {
            Push::chain_deleted { gone } => {
                self.held.close(&gone.id, &mut self.out);
                for page in self.tabs.values_mut() {
                    page.opened.close(&gone.id, &mut self.out);
                }
            }
            Push::event_read { event } => {
                for page in self.tabs.values_mut() {
                    page.event.put(event, record.clone(), &mut self.out);
                }
            }
            Push::assistant_feed_updated { .. } => self.assistant.set_feed(record.clone(), &mut self.out),
            // What streamed in the home was let go as this session fell behind: it is read again, and what is pushed
            // for it meanwhile waits for the answer.
            Push::home_reset { reset } => {
                self.calls
                    .tally
                    .repair(self.calls.now, RepairReason::Reset, &reset.home);
                self.held.read_again(&reset.home);
                let tabs: Vec<Tab> = self
                    .tabs
                    .iter()
                    .filter(|(_, p)| p.opened.home() == Some(reset.home.as_str()))
                    .map(|(tab, _)| *tab)
                    .collect();
                for tab in tabs {
                    self.open_chain(tab, reset.home.clone(), None);
                }
                if self.assistant.home() == Some(reset.home.as_str()) {
                    self.assistant.reload();
                }
            }
            _ => {
                self.held.on_push(push, record, &mut self.out);
                for page in self.tabs.values_mut() {
                    page.opened.on_push(push, record, &mut self.out);
                    page.repo.on_push(push, record, &mut self.out);
                    page.repos.on_push(push, record, &mut self.out);
                }
                self.assistant.on_push(push, record, &mut self.out);
            }
        }
    }

    fn shell_call(&mut self, asker: Asker, retry: Option<Retry>, call: Value) {
        if !self.calls.live() {
            self.answer(asker, Err(Failure::new(ErrorCode::Disconnected, OFFLINE)));
            return;
        }
        let kind = match kind_of(&call) {
            Ok(kind) => kind,
            Err(failure) => return self.answer(asker, Err(failure)),
        };
        let (request, retry) = if kind == CallKind::Change {
            let (place, key) = retry.map_or((String::new(), String::new()), |r| (r.place, r.key));
            let request = self.request_for(asker.tab, &place, &key);
            (Some(request.clone()), Some((place, key, request)))
        } else {
            (None, None)
        };
        match call_of(call, request.as_ref()) {
            Ok(call) => self
                .calls
                .ask(Outgoing::<Call, Value>::new(call), Purpose::Shell { asker, retry }),
            Err(failure) => self.answer(asker, Err(failure)),
        }
    }

    /// The request of a tab's place's action: the one kept when the same key is asked again after no answer came, else
    /// new. Places are named by each tab, so two tabs' places of the same name are two.
    fn request_for(&mut self, tab: Tab, place: &str, key: &str) -> Request {
        match self.unsettled.get(&(tab, place.to_owned())) {
            Some((held, request)) if !place.is_empty() && held == key => request.clone(),
            _ => self.calls.request(),
        }
    }

    fn remember(&mut self, tab: Tab, place: String, key: String, request: Request, result: &Result<Value, Failure>) {
        if place.is_empty() {
            return;
        }
        if unanswered(result) {
            self.unsettled.insert((tab, place), (key, request));
        } else {
            self.unsettled.remove(&(tab, place));
        }
    }

    /// An outbox send whose drafts' saves are answered: asked of the server with its request, or, when what was typed
    /// is not saved, held to wait for the session (none now) or failed.
    fn send_ready(&mut self, ready: Ready) {
        let Some(going) = self.outbox.going(&ready.send) else {
            return;
        };
        if !ready.saved {
            if self.calls.live() {
                let failure = Failure::new(
                    ErrorCode::Failed,
                    "what you typed is not saved yet; try again in a moment",
                );
                self.send_failed(&ready.send, &failure, false);
            } else {
                self.outbox.wait(&ready.send, &mut self.out);
            }
            return;
        }
        match call_of(going.call, Some(&going.request)) {
            Ok(call) => {
                self.drafts.sending(&going.ids);
                let purpose = Purpose::Send { id: ready.send };
                self.calls.ask(Outgoing::<Call, Value>::new(call), purpose);
            }
            Err(failure) => self.send_failed(&ready.send, &failure, false),
        }
    }

    fn send_failed(&mut self, id: &str, failure: &Failure, answered: bool) {
        if let Some(asker) = self.outbox.failed(id, failure, answered, &mut self.out) {
            self.answer(asker, Err(failure.clone()));
        }
    }

    /// The server answered an outbox send, or its session went first: it went, it waits for the next session with
    /// the same request, or it failed.
    fn sent(&mut self, id: &str, result: Result<Value, Failure>) {
        let ids = self.outbox.ids(id);
        let went = result.is_ok();
        let mut homes = Homes {
            chains: &mut self.held,
            assistant: &mut self.assistant,
        };
        self.drafts.sent(&mut self.calls, &mut homes, &ids, went, &mut self.out);
        match result {
            Ok(value) => {
                if let Some(asker) = self.outbox.went(id, self.calls.now, &mut self.out) {
                    self.answer(asker, Ok(value));
                }
            }
            Err(f) if f.code == ErrorCode::Disconnected => self.outbox.wait(id, &mut self.out),
            Err(f) => {
                let answered = f.code != ErrorCode::Timeout;
                self.send_failed(id, &f, answered);
            }
        }
    }

    /// Answers the tab that asked, if it is still there: no other tab hears of it.
    fn answer(&mut self, asker: Asker, result: Result<Value, Failure>) {
        if !self.tabs.contains_key(&asker.tab) {
            return;
        }
        let ticket = asker.ticket;
        let told = match result {
            Ok(value) => Told::Answer { ticket, value },
            Err(failure) => Told::Failed { ticket, failure },
        };
        self.outputs.push(Output::Tell { tab: asker.tab, told });
    }

    /// Counts a call that failed as a fault of the page, with the chain it was for or its tab showed: one the link being
    /// down failed, or the page let go of, or that found its record moved (the caller goes on from it), is no fault.
    fn call_failed(&mut self, purpose: &Purpose, method: &str, f: &Failure) {
        if matches!(
            f.code,
            ErrorCode::Disconnected | ErrorCode::Disposed | ErrorCode::Aborted | ErrorCode::Conflict
        ) || *purpose == Purpose::LinkReport
        {
            return;
        }
        let shown = |tab: &Tab| self.tabs.get(tab).and_then(|p| p.opened.shown()).unwrap_or_default();
        let chain = match purpose {
            Purpose::ChainGet { chain, .. } | Purpose::ChainEvents { chain, .. } => chain.as_str(),
            Purpose::Shell { asker, .. } => shown(&asker.tab),
            Purpose::CardStream { tab, .. } | Purpose::CardEarlier { tab, .. } | Purpose::EventGet { tab, .. } => {
                shown(tab)
            }
            _ => "",
        }
        .to_owned();
        let now = self.calls.now;
        self.calls.tally.fault(now, FaultKind::Call, &f.message, method, &chain);
    }

    fn on_answer(&mut self, purpose: Purpose, result: Result<Value, Failure>) {
        match purpose {
            Purpose::Shell { asker, retry } => {
                if let Some((place, key, request)) = retry {
                    self.remember(asker.tab, place, key, request, &result);
                }
                self.answer(asker, result);
            }
            Purpose::Send { id } => self.sent(&id, result),
            Purpose::LinkReport => {}
            Purpose::Snapshot { attempt } => {
                if attempt == self.sync.attempt && matches!(self.sync.stage, Stage::Snapshot { .. }) {
                    self.snapshot_answered(result);
                }
            }
            // A failure brings no mark: the homes waiting for this telling's are read, the session being pushed every
            // home (it was never narrowed) or gone (its reads fail as well, and the next session names them anew).
            Purpose::HomesShown { seq } => {
                if result.is_err() {
                    self.shown.marks.retain(|_, wanted| *wanted != seq);
                    self.read_shown();
                }
            }
            Purpose::ChainGet { ref chain, seq } | Purpose::ChainEvents { ref chain, seq } => {
                let view = matches!(purpose, Purpose::ChainGet { .. });
                let Some(read) = self
                    .held
                    .answered(chain, seq, result, view, &mut self.catalog, &mut self.out)
                else {
                    return;
                };
                // Every tab opening the chain waits for this one read.
                let tabs: Vec<Tab> = self
                    .tabs
                    .iter()
                    .filter(|(_, p)| p.opened.opening == *chain)
                    .map(|(tab, _)| *tab)
                    .collect();
                for tab in tabs {
                    self.chain_read(tab, read.as_ref());
                }
            }
            Purpose::CardStream { tab, card, generation } => {
                if let Some(page) = self.tabs.get_mut(&tab) {
                    page.opened.stream_answered(card, generation, result, &mut self.out);
                }
            }
            Purpose::CardEarlier { tab, card, generation } => {
                if let Some(page) = self.tabs.get_mut(&tab) {
                    page.opened.earlier_answered(card, generation, result, &mut self.out);
                }
            }
            // One view failing never fails the sync: the store stays as it is, the owner is told, and the next
            // (re)connect reads it again.
            Purpose::AtWork { attempt } => {
                let done = self.catalog.at_work_answered(result, &mut self.out);
                self.could_not(None, What::LoadAtWork, done);
                self.reloaded(attempt, Ok(()));
            }
            Purpose::AssistantStream { seq } => {
                if let Some(done) = self.assistant.answered(seq, result, &mut self.out) {
                    match self.sync.assistant.take() {
                        Some(attempt) => self.reloaded(attempt, done),
                        None => self.could_not(None, What::LoadAssistant, done),
                    }
                }
            }
            Purpose::EventGet { tab, seq } => {
                let Some(page) = self.tabs.get_mut(&tab) else { return };
                if let Some((by_sync, done)) = page.event.answered(seq, result, &mut self.out) {
                    match by_sync {
                        Some(attempt) => self.reloaded(attempt, done),
                        None => self.could_not(Some(tab), What::OpenEvent, done),
                    }
                }
            }
            Purpose::Archived { tab, generation } => {
                if let Some(page) = self.tabs.get_mut(&tab) {
                    page.archived
                        .answered(generation, result, &mut self.catalog, &mut self.out);
                }
            }
            Purpose::Repos { tab, every, generation } => {
                if let Some(page) = self.tabs.get_mut(&tab) {
                    let repos = if every { &mut page.repos } else { &mut page.repo };
                    repos.answered(generation, result, &mut self.out);
                }
            }
            Purpose::DraftSave(saving) => self.draft_answered(saving, result),
            Purpose::Discard { id } => self
                .drafts
                .discard_answered(&mut self.calls, &id, result, &mut self.out),
        }
    }

    fn draft_answered(&mut self, saving: Saving, result: Result<Value, Failure>) {
        let mut homes = Homes {
            chains: &mut self.held,
            assistant: &mut self.assistant,
        };
        self.drafts
            .save_answered(&mut self.calls, &mut homes, saving, result, &mut self.out);
    }

    fn chain_opened(&mut self, tab: Tab, opened: Opened) {
        match opened.by_sync {
            Some(attempt) => self.reloaded(attempt, opened.result),
            None => self.could_not(Some(tab), What::OpenChain, opened.result),
        }
    }

    /// Something the owner asked for did not happen: told to the tab that asked, or, for what every tab shares, where
    /// he looks.
    fn could_not(&mut self, tab: Option<Tab>, what: What, result: Result<(), Failure>) {
        if let Err(failure) = result {
            self.out.notices.push((tab, Notice::CouldNot { what, failure }));
        }
    }

    /// The state tab `page` is told: the link's, shared, and what it opened.
    fn state(&self, page: &Page) -> State {
        State {
            phase: self.phase,
            why: self.why.clone(),
            banner: banner::banner(&banner::Link {
                now: self.calls.now,
                phase: self.phase,
                why: &self.why,
                refused: self.refused,
                lost_at: self.lost_at,
                tries: self.redials,
                next_try: self.redial_at,
                back_until: self.back_until,
                waiting: self.outbox.waiting(),
            }),
            blocked: banner::blocked(self.phase),
            refused: self.refused,
            loaded: self.catalog.loaded,
            opening: page.opened.opening.clone(),
            truncated: self.held.truncated(page.opened.shown()),
            earlier: page.opened.earlier.iter().cloned().collect(),
            assistant_loaded: self.assistant.loaded,
            event_opening: page.event.opening.clone(),
            archived_generation: page.archived.generation,
            archived_read: page.archived.read,
            archived_more: page.archived.more(),
            repos_read: page.repos.read,
        }
    }

    fn next_deadline(&self) -> Option<Millis> {
        let sync = match self.sync.stage {
            Stage::Idle => None,
            Stage::Snapshot { deadline } | Stage::Reloads { deadline, .. } => Some(deadline),
            Stage::Waiting { until } => Some(until),
        };
        [
            self.calls.conn.as_ref().and_then(Conn::next_deadline),
            sync,
            self.drafts.next_deadline(),
            self.outbox.next_deadline(),
            self.dialling,
            self.redial_at,
            self.redial_at.and_then(|at| banner::next_count(at, self.calls.now)),
            self.back_until.filter(|until| *until > self.calls.now),
            self.calls.tally.due().filter(|_| self.calls.live()),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    /// The tabs a notice goes to: the one it is for; every tab for where duels stand, which a page reads again; else
    /// the tabs the owner sees, or every tab when he sees none.
    fn audience(&self, to: Option<Tab>, notice: &Notice) -> Vec<Tab> {
        if let Some(tab) = to {
            return self.tabs.contains_key(&tab).then_some(tab).into_iter().collect();
        }
        let every = || self.tabs.keys().copied().collect();
        if matches!(notice, Notice::DuelsMoved) {
            return every();
        }
        let seen: Vec<Tab> = self.tabs.iter().filter(|(_, p)| !p.hidden).map(|(t, _)| *t).collect();
        if seen.is_empty() { every() } else { seen }
    }

    /// Ends the delay of each event pushed this step that a tab is handed now (decision 12): once, at the first tab.
    /// An event no tab is handed in the step it arrived (no tab shows it, or its home is being read) is not counted.
    fn count_handed(&mut self, told: &[(Tab, Told)]) {
        let pushed_at = std::mem::take(&mut self.pushed_at);
        if pushed_at.is_empty() {
            return;
        }
        let streams = |m: &Model| matches!(m, Model::ChainEvents | Model::CardEvents(_) | Model::AssistantEvents);
        let mut handed: Vec<&Key> = Vec::new();
        for (_, t) in told {
            match t {
                Told::Rows { changes } => handed.extend(changes.iter().filter_map(|c| match c {
                    Change::Put { model, id, .. } if streams(model) => Some(id),
                    _ => None,
                })),
                Told::Model { model, rows, .. } if streams(model) => handed.extend(rows.iter().map(|r| &r.id)),
                _ => {}
            }
        }
        let now = self.calls.now;
        let mut counted = std::collections::HashSet::new();
        for id in handed {
            if let Some(at) = pushed_at.get(id)
                && counted.insert(id)
            {
                self.calls.tally.delay(now, *at);
            }
        }
    }

    /// What each tab is told this step: the answers, what changed in the models it shows, what the owner should
    /// notice, what to keep, and its state when it moved; then the timer when it moved.
    fn flush(&mut self) -> Vec<Output> {
        let mut outputs = std::mem::take(&mut self.outputs);
        let out = std::mem::take(&mut self.out);
        self.fresh.update(&fresh::Link {
            now: self.calls.now,
            phase: self.phase,
            settled: self.catalog.loaded || self.refused.is_some(),
            lost_at: self.lost_at,
            assistant_read: self.assistant.loading,
            tabs: self
                .tabs
                .iter()
                .map(|(tab, p)| Reads {
                    tab: *tab,
                    chain: !p.opened.opening.is_empty(),
                    event: !p.event.opening.is_empty(),
                })
                .collect(),
        });
        let fresh = &self.fresh;
        let told = self.models.apply(out.patches, |tab, model| fresh.of(tab, model));
        self.count_handed(&told);
        outputs.extend(told.into_iter().map(|(tab, told)| Output::Tell { tab, told }));
        for (to, notice) in out.notices {
            for tab in self.audience(to, &notice) {
                let told = Told::Notice { notice: notice.clone() };
                outputs.push(Output::Tell { tab, told });
            }
        }
        if let Some(kept) = self.outbox.kept(&self.drafts) {
            for tab in self.tabs.keys() {
                let told = Told::Keep { kept: kept.clone() };
                outputs.push(Output::Tell { tab: *tab, told });
            }
        }
        let states: Vec<(Tab, State)> = self.tabs.iter().map(|(tab, p)| (*tab, self.state(p))).collect();
        for (tab, state) in states {
            let Some(page) = self.tabs.get_mut(&tab) else { continue };
            if page.last_state.as_ref() != Some(&state) {
                page.last_state = Some(state.clone());
                outputs.push(Output::Tell {
                    tab,
                    told: Told::State { state },
                });
            }
        }
        let timer = self.next_deadline();
        if self.last_timer != Some(timer) {
            self.last_timer = Some(timer);
            outputs.push(Output::Timer(timer));
        }
        outputs
    }
}
