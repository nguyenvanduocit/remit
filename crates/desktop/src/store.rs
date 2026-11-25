//! What the window holds of the core (ADR-0128 decision 4): each model it draws, as the rows the core told it, and the
//! core's state, in signals a screen reads; what the owner should notice goes where he looks. A screen asks for a
//! model by reading it: the first read shows it (`Ask::Show`), and the core tells it whole, then each change. Rows stay
//! the server's JSON until a screen decodes the ones it draws into the `model` and `wire` types they are.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use gui::Signal;
use link::Failure;
use replica::{Ask, Change, Kept, Key, Model, Notice, Row, State, Ticket, Told};
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Hands an ask to the core: over the shell thread's channel, or to the trace's own pump.
pub type Asker = Rc<dyn Fn(Ask)>;

/// Keeps what the core says to keep across a restart (`Told::Keep`): the owner's sends the server has not taken.
pub type Keeper = Rc<dyn Fn(&Kept)>;

/// Says what the owner should notice (`Told::Notice`) where he looks.
pub type Noticer = Rc<dyn Fn(Notice)>;

/// Takes the answer to a call: the server's JSON, or why it failed.
type Answered = Box<dyn FnOnce(Result<Value, Failure>)>;

#[derive(Clone)]
pub struct Store(Rc<Inner>);

pub struct WeakStore(std::rc::Weak<Inner>);

impl WeakStore {
    pub fn upgrade(&self) -> Option<Store> {
        self.0.upgrade().map(Store)
    }
}

struct Inner {
    ask: Asker,
    keep: Keeper,
    notice: Noticer,
    models: RefCell<BTreeMap<Model, Shown>>,
    state: Signal<Option<State>>,
    tickets: Cell<Ticket>,
    answers: RefCell<HashMap<Ticket, Answered>>,
}

/// The rows of one model the window draws.
type Shown = Signal<Rc<Vec<Row>>>;

impl Store {
    pub fn new(ask: Asker, keep: Keeper, notice: Noticer) -> Self {
        Self(Rc::new(Inner {
            ask,
            keep,
            notice,
            models: RefCell::new(BTreeMap::new()),
            state: Signal::labeled("core state", None),
            tickets: Cell::new(0),
            answers: RefCell::new(HashMap::new()),
        }))
    }

    /// A handle that does not keep the store: the mailbox's, so a window that comes in again lets the old core go.
    pub fn downgrade(&self) -> WeakStore {
        WeakStore(Rc::downgrade(&self.0))
    }

    pub fn ask(&self, ask: Ask) {
        (self.0.ask)(ask);
    }

    /// The model, shown from its first read on.
    fn shown(&self, model: &Model) -> Shown {
        if let Some(shown) = self.0.models.borrow().get(model) {
            return shown.clone();
        }
        let shown = Signal::new(Rc::new(Vec::new()));
        self.0.models.borrow_mut().insert(model.clone(), shown.clone());
        self.ask(Ask::Show { model: model.clone() });
        shown
    }

    /// The window no longer draws `model`: it is told nothing more of it.
    pub fn hide(&self, model: &Model) {
        if self.0.models.borrow_mut().remove(model).is_some() {
            self.ask(Ask::Hide { model: model.clone() });
        }
    }

    /// Calls the server with `call` (a `wire::app::Call` as JSON: `{ method, args }`); `then` takes its answer. The core
    /// gives a changing call its request id.
    pub fn call(&self, call: Value, then: impl FnOnce(Result<Value, Failure>) + 'static) {
        let ticket = self.ticket(then);
        self.ask(Ask::Call {
            ticket,
            retry: None,
            call,
        });
    }

    /// Sends the owner's drafts `ids` with `call` through the core's outbox (ADR-0094): shown at once at `at` as `text`,
    /// kept while the link is down, sent once; `then` takes the answer.
    pub fn send(
        &self,
        ids: Vec<Key>,
        call: Value,
        at: Key,
        text: String,
        then: impl FnOnce(Result<Value, Failure>) + 'static,
    ) {
        let ticket = self.ticket(then);
        self.ask(Ask::Send {
            ticket,
            ids,
            call,
            at,
            text,
        });
    }

    fn ticket(&self, then: impl FnOnce(Result<Value, Failure>) + 'static) -> Ticket {
        let ticket = self.0.tickets.get().wrapping_add(1);
        self.0.tickets.set(ticket);
        self.0.answers.borrow_mut().insert(ticket, Box::new(then));
        ticket
    }

    /// The rows of `model`, in its order.
    pub fn rows(&self, model: &Model) -> Rc<Vec<Row>> {
        self.shown(model).get()
    }

    /// The records of `model` as `T`, in its order; a row that does not read as `T` is said on stderr and left out.
    pub fn records<T: DeserializeOwned>(&self, model: &Model) -> Vec<T> {
        self.rows(model).iter().filter_map(|row| decode(model, row)).collect()
    }

    /// The one record of a model of one record (`Who`, `Board`, …), as `T`.
    pub fn one<T: DeserializeOwned>(&self, model: &Model) -> Option<T> {
        self.rows(model).first().and_then(|row| decode(model, row))
    }

    pub fn state(&self) -> Option<State> {
        self.0.state.get()
    }

    /// Takes in what the core told the window, in order, as one change to what it draws.
    pub fn apply(&self, told: Vec<Told>) {
        gui::batch(|| told.into_iter().for_each(|t| self.take(t)));
    }

    fn take(&self, told: Told) {
        match told {
            Told::Model { model, rows, .. } => {
                if let Some(shown) = self.0.models.borrow().get(&model) {
                    shown.set(Rc::new(rows));
                }
            }
            Told::Rows { changes } => changes.into_iter().for_each(|c| self.change(c)),
            Told::State { state } => self.0.state.set(Some(state)),
            // How fresh each model is shows in the link's banner.
            Told::Status { .. } => {}
            Told::Answer { ticket, value } => self.answer(ticket, Ok(value)),
            Told::Failed { ticket, failure } => self.answer(ticket, Err(failure)),
            Told::Notice { notice } => (self.0.notice)(notice),
            Told::Keep { kept } => (self.0.keep)(&kept),
        }
    }

    fn answer(&self, ticket: Ticket, result: Result<Value, Failure>) {
        let then = self.0.answers.borrow_mut().remove(&ticket);
        if let Some(then) = then {
            then(result);
        }
    }

    fn change(&self, change: Change) {
        let model = match &change {
            Change::Put { model, .. } | Change::Append { model, .. } | Change::Drop { model, .. } => model,
        };
        let Some(shown) = self.0.models.borrow().get(model).cloned() else {
            return;
        };
        shown.update(|mut rows| {
            apply(Rc::make_mut(&mut rows), change);
            rows
        });
    }
}

/// One change to a model's rows, as the web's page applies it (`core/client.ts`).
fn apply(rows: &mut Vec<Row>, change: Change) {
    match change {
        Change::Put { id, before, record, .. } => {
            if let Some(row) = rows.iter_mut().find(|r| r.id == id) {
                row.record = record;
                return;
            }
            let at = before
                .and_then(|b| rows.iter().position(|r| r.id == b))
                .unwrap_or(rows.len());
            rows.insert(at, Row { id, record });
        }
        // A streamed chunk grows the text of the event the row holds (an `EventRead`'s `event`).
        Change::Append { id, text, .. } => {
            let Some(row) = rows.iter_mut().find(|r| r.id == id) else {
                return;
            };
            if let Some(event) = row.record.get_mut("event").and_then(Value::as_object_mut) {
                let grown = format!(
                    "{}{text}",
                    event.get("text").and_then(Value::as_str).unwrap_or_default()
                );
                event.insert("text".to_owned(), Value::String(grown));
            }
        }
        Change::Drop { id, .. } => rows.retain(|r| r.id != id),
    }
}

fn decode<T: DeserializeOwned>(model: &Model, row: &Row) -> Option<T> {
    match T::deserialize(&row.record) {
        Ok(record) => Some(record),
        Err(e) => {
            eprintln!("remit-desktop: a row {} of {model:?} does not read: {e}", row.id);
            None
        }
    }
}
