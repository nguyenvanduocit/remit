//! The SharedWorker's entry (ADR-0095 decision 4): the browser's one core, serving each tab that connects on its port.
//! The worker's script hands every port it is given to `Tabs::connect`. A tab's first message is its hello (how the core
//! starts, if this tab is the first; the credential the next dial carries; the Web Lock it holds while it is open); then
//! its asks; `bye` when it lets the core go. A tab that goes without saying so (closed, reloaded, crashed) releases its
//! lock, and the worker, waiting for that lock, lets the tab go then.

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::Function;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{MessageEvent, MessagePort, WorkerGlobalScope};

use crate::js::Start;
use crate::port::FromTab;
use crate::shell::{Core, Sink, Tab, console_error, parse};

/// How the core is made once the first tab says how.
type Make = Box<dyn Fn(&Start, Function) -> Core>;

struct Shared {
    make: Make,
    core: RefCell<Option<Core>>,
    /// The credential the newest hello carried: the one every dial carries from then on.
    credential: Rc<RefCell<String>>,
    /// Hands the dialer `credential`.
    give_credential: Closure<dyn Fn() -> JsValue>,
}

impl Shared {
    fn core(&self, start: &str) -> Result<(), JsError> {
        if self.core.borrow().is_some() {
            return Ok(());
        }
        let start: Start = parse("the start", start)?;
        let core = (self.make)(
            &start,
            self.give_credential.as_ref().unchecked_ref::<Function>().clone(),
        );
        *self.core.borrow_mut() = Some(core);
        Ok(())
    }
}

/// The SharedWorker's core and the tabs it serves.
#[wasm_bindgen]
pub struct Tabs {
    shared: Rc<Shared>,
}

impl Tabs {
    fn new(make: Make) -> Self {
        let credential = Rc::new(RefCell::new(String::new()));
        let held = Rc::clone(&credential);
        Self {
            shared: Rc::new(Shared {
                make,
                core: RefCell::new(None),
                credential,
                give_credential: Closure::new(move || JsValue::from_str(&held.borrow())),
            }),
        }
    }
}

/// A tab's handle, once it said hello.
type Served = Rc<RefCell<Option<Tab>>>;

/// Lets the tab go (`Input::Left`) and its port: with no handler on it, nothing holds the handler, which the browser
/// collects with what it holds (the port, the tab's handle).
fn let_go(served: &Served, port: &MessagePort) {
    served.borrow_mut().take();
    port.set_onmessage(None);
    port.close();
}

/// Lets the tab go once the Web Lock it holds is released: it is gone.
fn when_gone(lock: &str, served: &Served, port: &MessagePort) -> Result<(), JsValue> {
    let scope: WorkerGlobalScope = js_sys::global().dyn_into()?;
    let served = Rc::clone(served);
    let port = port.clone();
    let gone = Closure::once_into_js(move |_: JsValue| let_go(&served, &port));
    let locks = scope.navigator().locks();
    let _granted = locks.request(lock, gone.unchecked_ref());
    Ok(())
}

#[wasm_bindgen]
impl Tabs {
    /// The worker of the app: the first tab's hello starts a core over the browser's WebTransport. `link` is called
    /// before each dial and returns where the link is now (`url` and `certificateHashes`, as the server's `/link.json`
    /// says them), or a promise of it: a server that restarted presents a new certificate.
    pub fn browser(link: Function) -> Tabs {
        Self::new(Box::new(move |start, credential| {
            Core::browser(start, link.clone(), credential)
        }))
    }

    /// The worker of the verify pages: the first tab's hello starts a core over `server`, the fake server's half.
    #[cfg(feature = "fake")]
    pub fn fake(server: &crate::fake::FakeServer) -> Tabs {
        let server = server.clone();
        Self::new(Box::new(move |start, _| Core::faked(start, &server)))
    }

    /// Serves the tab on `port`. A message that is none of a tab's (the verify pages' seed traffic) is another
    /// listener's, and left to it.
    pub fn connect(&self, port: MessagePort) {
        let shared = Rc::clone(&self.shared);
        let served: Served = Rc::new(RefCell::new(None));
        let to = port.clone();
        // The handler is the port's alone: wasm-bindgen frees it once the browser collects it, never while it runs.
        let on = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
            let Ok(said) = serde_wasm_bindgen::from_value::<FromTab>(e.data()) else {
                return;
            };
            match said {
                FromTab::Hello {
                    start,
                    credential,
                    lock,
                } => {
                    *shared.credential.borrow_mut() = credential;
                    if let Err(e) = shared.core(&start) {
                        console_error(&e.into());
                        return;
                    }
                    let tab = shared
                        .core
                        .borrow()
                        .as_ref()
                        .map(|core| core.join(Sink::Port(to.clone())));
                    *served.borrow_mut() = tab;
                    if let Err(e) = when_gone(&lock, &served, &to) {
                        console_error(&e);
                    }
                }
                FromTab::Ask { ask } => {
                    if let Some(tab) = served.borrow().as_ref() {
                        tab.give(ask);
                    }
                }
                FromTab::Bye => let_go(&served, &to),
            }
        });
        port.set_onmessage(Some(on.into_js_value().unchecked_ref()));
    }
}
