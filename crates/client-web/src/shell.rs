//! The browser's core: it owns the `Replica`, steps it one input at a time with the page's clock, and carries out what
//! each step says on the transport (`Wire`), on its one timer and to each tab it serves (`Told`, through that tab's
//! `Sink`). One core serves every tab of the browser (ADR-0095 decision 4); a `Tab` is one tab's handle on it.
//!
//! Inputs queue: what the transport, the timer or a tab hands in while a step runs (a fake server answers at once, a
//! sink may ask again) waits for that step to end, so the core is never entered twice and every input is taken in the
//! order it came.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::rc::{Rc, Weak};

use js_sys::Function;
use link::{Millis, Transport};
use replica::{Ask, Input, Output, Replica, Told};
use serde::Serialize;
use serde::de::DeserializeOwned;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

use crate::js::Start;

#[wasm_bindgen]
extern "C" {
    // The global timers, in the browser and in Node alike.
    #[wasm_bindgen(js_name = setTimeout)]
    fn set_timeout(handler: &Function, ms: i32) -> JsValue;
    #[wasm_bindgen(js_name = clearTimeout)]
    fn clear_timeout(id: &JsValue);
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    pub(crate) fn console_error(message: &JsValue);
    /// The page's or worker's precise clock, in milliseconds.
    #[wasm_bindgen(js_namespace = performance, js_name = now)]
    pub(crate) fn precise_now() -> f64;
}

/// The page's clock, in milliseconds.
pub(crate) fn now() -> Millis {
    #[expect(
        clippy::as_conversions,
        reason = "Date.now() is a whole count of milliseconds since 1970; `as` saturates and no From exists for f64"
    )]
    let ms = js_sys::Date::now() as Millis;
    ms
}

/// A transport the core dials and carries the link's streams on: the browser's WebTransport, or the fake server.
pub(crate) trait Wire: Transport {
    /// Opens a session; the wire then hands in `Opened` or `DialFailed`.
    fn dial(&mut self);
}

/// Where a tab is told what the core says to it, as plain objects: a function the page gave (a tab in the core's own
/// page), or the port of a tab the SharedWorker serves.
#[derive(Clone)]
pub(crate) enum Sink {
    Call(Function),
    #[cfg(target_arch = "wasm32")]
    Port(web_sys::MessagePort),
}

impl Sink {
    fn tell(&self, told: &JsValue) -> Result<(), JsValue> {
        match self {
            Self::Call(f) => f.call1(&JsValue::NULL, told).map(drop),
            #[cfg(target_arch = "wasm32")]
            Self::Port(port) => port.post_message(told),
        }
    }
}

/// Hands an input to the core; once the core is gone, nothing.
#[derive(Clone)]
pub(crate) struct Feed(Weak<Inner>);

impl Feed {
    pub(crate) fn give(&self, input: Input) {
        if let Some(inner) = self.0.upgrade() {
            inner.give(input);
        }
    }

    fn woke(&self) {
        if let Some(inner) = self.0.upgrade() {
            inner.woke();
        }
    }
}

struct Timer {
    id: JsValue,
    at: Millis,
}

pub(crate) struct Inner {
    core: RefCell<Replica>,
    wire: RefCell<Box<dyn Wire>>,
    queue: RefCell<VecDeque<Input>>,
    busy: Cell<bool>,
    /// The tabs served, each by its number.
    sinks: RefCell<BTreeMap<replica::Tab, Sink>>,
    next_tab: Cell<replica::Tab>,
    timer: RefCell<Option<Timer>>,
    wake: Closure<dyn FnMut()>,
}

impl Inner {
    fn give(&self, input: Input) {
        self.queue.borrow_mut().push_back(input);
        if self.busy.replace(true) {
            return;
        }
        loop {
            let next = self.queue.borrow_mut().pop_front();
            let Some(input) = next else {
                break;
            };
            self.step(input);
        }
        self.busy.set(false);
    }

    fn step(&self, input: Input) {
        let outputs = self.core.borrow_mut().handle(now(), input);
        let mut told: BTreeMap<replica::Tab, Vec<Told>> = BTreeMap::new();
        for output in outputs {
            match output {
                Output::Tell { tab, told: t } => told.entry(tab).or_default().push(t),
                other => self.carry_out(other),
            }
        }
        for (tab, told) in told {
            self.tell(tab, &told);
        }
    }

    /// Tells `tab` one step's batch, as plain objects and numbers, as the JSON the page read before: maps as objects,
    /// `None` as `null`.
    fn tell(&self, tab: replica::Tab, told: &[Told]) {
        // Taken out first: a sink may let its tab go while it is told.
        let Some(sink) = self.sinks.borrow().get(&tab).cloned() else {
            return;
        };
        let page = serde_wasm_bindgen::Serializer::json_compatible();
        match told.serialize(&page) {
            // A sink that throws is the page's fault; the core goes on, and the console says so.
            Ok(told) => {
                if let Err(e) = sink.tell(&told) {
                    console_error(&e);
                }
            }
            Err(e) => console_error(&JsValue::from_str(&format!("client-web: tell a tab: {e}"))),
        }
    }

    fn carry_out(&self, output: Output) {
        match output {
            Output::Dial => self.wire.borrow_mut().dial(),
            Output::Open(stream) => self.wire.borrow_mut().open(stream),
            Output::Send(stream, bytes) => self.wire.borrow_mut().send(stream, bytes),
            Output::Finish(stream) => self.wire.borrow_mut().finish(stream),
            Output::Reset(stream) => self.wire.borrow_mut().reset(stream),
            Output::Close(closing) => self.wire.borrow_mut().close(closing),
            Output::Timer(at) => self.arm(at),
            // A tab's, told by `step`.
            Output::Tell { .. } => {}
        }
    }

    /// Wakes the core at `at`, in place of any time asked before.
    fn arm(&self, at: Option<Millis>) {
        let old = self.timer.borrow_mut().take();
        if let Some(old) = old {
            clear_timeout(&old.id);
        }
        if let Some(at) = at {
            let delay = i32::try_from(at.saturating_sub(now())).unwrap_or(i32::MAX);
            let id = set_timeout(self.wake.as_ref().unchecked_ref(), delay);
            *self.timer.borrow_mut() = Some(Timer { id, at });
        }
    }

    /// The timer fired; a timer that fires before its time (the clock moved) is armed again for the rest.
    fn woke(&self) {
        let fired = self.timer.borrow_mut().take();
        let Some(timer) = fired else {
            return;
        };
        if now() < timer.at {
            self.arm(Some(timer.at));
        } else {
            self.give(Input::Tick);
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        // The closure goes with the core: a timer left set would call it after.
        if let Some(timer) = self.timer.get_mut().take() {
            clear_timeout(&timer.id);
        }
    }
}

pub(crate) fn parse<T: DeserializeOwned>(what: &str, json: &str) -> Result<T, JsError> {
    serde_json::from_str(json).map_err(|e| JsError::new(&format!("client-web: read {what}: {e}")))
}

/// The client core of the browser, one for all its tabs; each tab draws through a `Tab` of it.
#[wasm_bindgen]
pub struct Core {
    inner: Rc<Inner>,
}

impl Core {
    fn start(start: &Start, wire: impl FnOnce(Feed) -> Box<dyn Wire>) -> Self {
        let inner = Rc::new_cyclic(|weak: &Weak<Inner>| {
            let feed = Feed(weak.clone());
            let waker = feed.clone();
            Inner {
                core: RefCell::new(Replica::new(start.options())),
                wire: RefCell::new(wire(feed)),
                queue: RefCell::new(VecDeque::new()),
                busy: Cell::new(false),
                sinks: RefCell::new(BTreeMap::new()),
                next_tab: Cell::new(0),
                timer: RefCell::new(None),
                wake: Closure::new(move || waker.woke()),
            }
        });
        Self { inner }
    }

    /// A core over the browser's WebTransport. Before each dial, `link` is called and returns where the link is now
    /// (`url` and `certificateHashes`, as the server's `/link.json`), and `credential` the credential that session's
    /// hello carries (text); either may return a promise of it.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn browser(start: &Start, link: Function, credential: Function) -> Self {
        let dialer = crate::browser::Dialer::new(link, credential);
        Self::start(start, move |feed| Box::new(dialer.wire(feed)))
    }

    #[cfg(feature = "fake")]
    pub(crate) fn faked(start: &Start, server: &crate::fake::FakeServer) -> Self {
        Self::start(start, |feed| Box::new(server.wire(feed)))
    }

    /// A tab served from now on, told through `sink`.
    pub(crate) fn join(&self, sink: Sink) -> Tab {
        let id = self.inner.next_tab.get().wrapping_add(1);
        self.inner.next_tab.set(id);
        self.inner.sinks.borrow_mut().insert(id, sink);
        Tab {
            inner: Rc::downgrade(&self.inner),
            id,
        }
    }
}

#[wasm_bindgen]
impl Core {
    /// A core whose transport is `server`, the fake server's half of the link.
    #[cfg(feature = "fake")]
    pub fn fake(start: &str, server: &crate::fake::FakeServer) -> Result<Core, JsError> {
        let start: Start = parse("the start", start)?;
        Ok(Self::faked(&start, server))
    }

    /// A tab of this page, told through `sink`: called with a `Told[]`, as plain objects, after each step that has
    /// something for it.
    pub fn tab(&self, sink: Function) -> Tab {
        self.join(Sink::Call(sink))
    }
}

/// One tab's handle on the core: what it asks goes in as its own, and once the handle is let go (`free`, or the
/// SharedWorker seeing the tab gone) the core lets go of what the tab opened, and closes the session with the last.
#[wasm_bindgen]
pub struct Tab {
    inner: Weak<Inner>,
    id: replica::Tab,
}

impl Tab {
    pub(crate) fn give(&self, ask: Ask) {
        if let Some(inner) = self.inner.upgrade() {
            inner.give(Input::Ask { tab: self.id, ask });
        }
    }
}

#[wasm_bindgen]
impl Tab {
    /// One of the tab's asks, an `Ask` as a plain object.
    pub fn ask(&self, ask: JsValue) -> Result<(), JsError> {
        let ask: Ask =
            serde_wasm_bindgen::from_value(ask).map_err(|e| JsError::new(&format!("client-web: read an ask: {e}")))?;
        self.give(ask);
        Ok(())
    }
}

impl Drop for Tab {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.upgrade() {
            inner.sinks.borrow_mut().remove(&self.id);
            inner.give(Input::Left { tab: self.id });
        }
    }
}
