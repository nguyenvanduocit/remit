//! The owner's words to one agent (ADR-0094, the web's `drafts.ts`): the server keeps his unsent words as a `Draft`, the
//! core holds what he typed that the server does not have yet (`Model::Typed`) and saves it after a pause; a send leaves the
//! box at once for a pending row (`Model::Outbox`) and goes when the link is up, with its request id, once.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use gui::{Effect, TextField};
use replica::{Ask, Model, Pending};
use serde_json::{Value, json};
use wire::app::DraftInput;

use crate::store::Store;

/// A box that holds one draft: to agent `card`, kept by its `home` (a chain, the Assistant), whose drafts the core
/// tells as `drafts`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DraftBox {
    pub home: String,
    pub card: String,
    pub drafts: Model,
}

impl DraftBox {
    /// Where the box's sends show: its name among the boxes of the window, as the web names it.
    pub fn key(&self) -> String {
        format!("{}:{}", self.home, self.card)
    }
}

/// The draft the box holds: its id and the words last written to it, and the drafts it sent. A sent draft never comes
/// back into the box by itself: the server's copy outlives the send until its deletion is pushed, and the core is on
/// another thread, so the window can read it between the two.
#[derive(Default)]
struct Held {
    open: Option<DraftBox>,
    id: String,
    last: String,
    sent: HashSet<String>,
}

pub struct Composer {
    pub field: TextField,
    held: Rc<RefCell<Held>>,
    /// Writes what the owner types; takes up the server's draft the box holds once it arrives.
    effects: RefCell<Vec<Effect>>,
    salt: String,
    made: Cell<u64>,
}

impl Composer {
    pub fn new(salt: String) -> Self {
        Self {
            field: TextField::new(""),
            held: Rc::new(RefCell::new(Held::default())),
            effects: RefCell::new(Vec::new()),
            salt,
            made: Cell::new(0),
        }
    }

    /// An id for a draft the server does not have yet.
    fn new_id(&self) -> String {
        let n = self.made.get().wrapping_add(1);
        self.made.set(n);
        format!("draft-{}-{n}", self.salt)
    }

    /// The box open, if any.
    pub fn open_box(&self) -> Option<DraftBox> {
        self.held.borrow().open.clone()
    }

    /// Opens `b`: its draft shows, and what the owner types in it is written from now on.
    pub fn open(&self, store: &Store, b: DraftBox) {
        if self.held.borrow().open.as_ref() == Some(&b) {
            return;
        }
        self.effects.borrow_mut().clear();
        let sent = std::mem::take(&mut self.held.borrow_mut().sent);
        let (id, text) = draft_in(store, &b, &sent).unwrap_or_else(|| (self.new_id(), String::new()));
        *self.held.borrow_mut() = Held {
            open: Some(b.clone()),
            id,
            last: text.clone(),
            sent,
        };
        self.field.set_text(text);
        let typing = {
            let (field, held, store, b) = (self.field.clone(), Rc::clone(&self.held), store.clone(), b.clone());
            Effect::new(move || {
                let text = field.text();
                let id = {
                    let mut h = held.borrow_mut();
                    if text == h.last {
                        return;
                    }
                    h.last.clone_from(&text);
                    h.id.clone()
                };
                store.ask(Ask::Write {
                    draft: json!({ "id": id, "home": b.home, "cardId": b.card, "text": text, "images": [] }),
                });
            })
        };
        // A box opened before the chain's drafts were told takes up the one the server keeps, while the owner has typed
        // nothing in it.
        let arriving = {
            let (field, held, store) = (self.field.clone(), Rc::clone(&self.held), store.clone());
            Effect::new(move || {
                let sent = held.borrow().sent.clone();
                let Some((id, text)) = draft_in(&store, &b, &sent) else {
                    return;
                };
                let adopt = {
                    let mut h = held.borrow_mut();
                    let untouched = h.last.is_empty() && h.id != id;
                    if untouched {
                        h.id = id;
                        h.last.clone_from(&text);
                    }
                    untouched
                };
                if adopt {
                    field.set_text(text);
                }
            })
        };
        self.effects.borrow_mut().extend([typing, arriving]);
    }

    /// Closes the box open: what it holds stays with the server and the core, and is shown when it opens again.
    pub fn close(&self) {
        self.effects.borrow_mut().clear();
        self.held.borrow_mut().open = None;
    }

    /// Sends the box's words to its agent by the call `deliver` makes of the draft's id (read after a turn in
    /// progress); the box empties for the next message. How the send goes shows in its pending row, a failure with why.
    pub fn send(&self, store: &Store, deliver: &dyn Fn(&str) -> Value) {
        let text = self.field.text();
        let words = text.trim().to_owned();
        let Some(b) = self.open_box() else { return };
        if words.is_empty() {
            return;
        }
        let id = {
            let mut h = self.held.borrow_mut();
            let id = std::mem::replace(&mut h.id, self.new_id());
            h.last = String::new();
            h.sent.insert(id.clone());
            id
        };
        store.send(vec![id.clone()], deliver(&id), b.key(), words, |_| {});
        self.field.set_text("");
    }

    /// Takes a send that failed back to edit: its draft is the box's again, its words in the box after any typed since.
    pub fn unsend(&self, store: &Store, p: &Pending) {
        store.ask(Ask::Unsend { id: p.id.clone() });
        let Some(id) = p.drafts.first().cloned() else { return };
        let typed = self.field.text();
        let text = if typed.trim().is_empty() {
            p.text.clone()
        } else {
            format!("{typed}\n\n{}", p.text)
        };
        {
            let mut h = self.held.borrow_mut();
            h.sent.remove(&id);
            h.id = id;
            // Written again by the typing effect, so the server's copy holds what the box shows.
            h.last = String::new();
        }
        self.field.set_text(text);
    }

    /// The box's sends on their way, oldest first.
    pub fn pending(&self, store: &Store) -> Vec<Pending> {
        let Some(b) = self.open_box() else {
            return Vec::new();
        };
        let at = b.key();
        store
            .records::<Pending>(&Model::Outbox)
            .into_iter()
            .filter(|p| p.at == at)
            .collect()
    }
}

/// The draft in box `b` and its words: what the owner typed that the server does not have yet, else the server's own;
/// none while he has written nothing there. A draft on its way, or sent from it, is not in its box.
fn draft_in(store: &Store, b: &DraftBox, sent: &HashSet<String>) -> Option<(String, String)> {
    let outgoing: Vec<String> = store
        .records::<Pending>(&Model::Outbox)
        .into_iter()
        .flat_map(|p| p.drafts)
        .collect();
    let mine = |home: &str, card: &str, id: &str| {
        home == b.home && card == b.card && !sent.contains(id) && !outgoing.iter().any(|o| o == id)
    };
    store
        .records::<DraftInput>(&Model::Typed)
        .into_iter()
        .find(|d| mine(&d.home, &d.card_id, &d.id))
        .map(|d| (d.id, d.text))
        .or_else(|| {
            store
                .records::<model::Draft>(&b.drafts)
                .into_iter()
                .find(|d| d.mentor.is_empty() && mine(&d.home, &d.card_id, &d.id))
                .map(|d| (d.id, d.text))
        })
}
