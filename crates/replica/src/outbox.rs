//! The owner's sends the server has not taken yet (ADR-0094). A send is a row here from the moment it is asked, so the
//! page shows it at once where its drafts were and never holds the box while it goes. With no session it waits and goes
//! when the session is back; its request id is given once, when it is asked, and kept for every try whose answer never
//! came, so the server, which answers a request id it has seen as it did the first time, acts on it once. Only a send
//! the server answered with a failure gets a new id when the owner tries it again.

use std::collections::HashMap;

use link::{Failure, Millis, Request};
use serde_json::Value;

use crate::calls::Calls;
use crate::drafts::Drafts;
use crate::index::Index;
use crate::records::{Asker, Out};
use crate::shell::{Kept, KeptSend, Key, Model, Pending, PendingStatus};

/// How long a send may go unanswered before the page says it is sending: a quicker answer shows no "sending…" at all.
pub(crate) const SLOW: Millis = 500;
/// How long a row the server took says it was sent before it goes: the message shows in its conversation by then.
pub(crate) const SENT_SHOWN: Millis = 2_500;

#[derive(Debug)]
struct Entry {
    /// The shell's call that asked it, until it is answered (it went, or failed).
    asker: Option<Asker>,
    ids: Vec<Key>,
    call: Value,
    at: Key,
    text: String,
    request: Request,
    /// The server answered `request`: a new try is a new action.
    answered: bool,
    status: PendingStatus,
    why: String,
    /// When a send asked of the server counts as slow, or a row that was sent goes.
    due: Option<Millis>,
}

/// A send the owner asked: the shell's call, its drafts and call, and where its row shows the words it sends.
pub(crate) struct Asked {
    pub(crate) asker: Asker,
    pub(crate) ids: Vec<Key>,
    pub(crate) call: Value,
    pub(crate) at: Key,
    pub(crate) text: String,
}

/// A send to ask of the server: its drafts, its call and the request it goes with.
pub(crate) struct Going {
    pub(crate) ids: Vec<Key>,
    pub(crate) call: Value,
    pub(crate) request: Request,
}

#[derive(Debug)]
pub(crate) struct Outbox {
    entries: HashMap<Key, Entry>,
    shown: Index<()>,
    /// A row changed since the shell was last told what to keep.
    changed: bool,
}

impl Default for Outbox {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            shown: Index::new(Model::Outbox),
            changed: false,
        }
    }
}

impl Outbox {
    /// Whether any send is held: a box's new draft waits for it (`Drafts::tick`).
    pub(crate) fn holds(&self) -> bool {
        self.entries.values().any(|e| e.status != PendingStatus::Sent)
    }

    /// How many sends wait for the link.
    pub(crate) fn waiting(&self) -> usize {
        self.entries
            .values()
            .filter(|e| e.status == PendingStatus::Waiting)
            .count()
    }

    /// The owner sent these drafts: a row, waiting to go. A failed row of any of the same drafts is let go: this send
    /// takes its place, and keeps its request when it sent the same drafts and its answer never came, as a retry.
    pub(crate) fn add(&mut self, calls: &mut Calls, asked: Asked, out: &mut Out) {
        let Asked {
            asker,
            ids,
            call,
            at,
            text,
        } = asked;
        let superseded: Vec<Key> = self
            .entries
            .iter()
            .filter(|(_, e)| e.status == PendingStatus::Failed && e.ids.iter().any(|d| ids.contains(d)))
            .map(|(id, _)| id.clone())
            .collect();
        let mut retried = None;
        for id in superseded {
            if let Some(e) = self.entries.remove(&id)
                && !e.answered
                && e.ids == ids
            {
                retried = Some(e.request);
            }
            self.hide(&id, out);
        }
        let request = retried.unwrap_or_else(|| calls.request());
        let id = request.id.clone();
        self.entries.insert(
            id.clone(),
            Entry {
                asker: Some(asker),
                ids,
                call,
                at,
                text,
                request,
                answered: false,
                status: PendingStatus::Waiting,
                why: String::new(),
                due: None,
            },
        );
        self.show(&id, out);
    }

    /// The rows waiting to go, oldest first, now asked of the server: what each sends.
    pub(crate) fn start(&mut self, now: Millis, out: &mut Out) -> Vec<(Key, Vec<Key>)> {
        let waiting: Vec<Key> = self
            .shown
            .keys()
            .map(|(id, ())| id)
            .filter(|id| {
                self.entries
                    .get(*id)
                    .is_some_and(|e| e.status == PendingStatus::Waiting)
            })
            .cloned()
            .collect();
        let mut started = Vec::with_capacity(waiting.len());
        for id in waiting {
            if let Some(e) = self.entries.get_mut(&id) {
                e.status = PendingStatus::Going;
                e.due = Some(now.saturating_add(SLOW));
                started.push((id.clone(), e.ids.clone()));
            }
            self.show(&id, out);
        }
        started
    }

    /// What a row asks of the server, once its drafts are saved; none for a row taken back meanwhile.
    pub(crate) fn going(&self, id: &str) -> Option<Going> {
        self.entries.get(id).map(|e| Going {
            ids: e.ids.clone(),
            call: e.call.clone(),
            request: e.request.clone(),
        })
    }

    /// The drafts a row sends.
    pub(crate) fn ids(&self, id: &str) -> Vec<Key> {
        self.entries.get(id).map(|e| e.ids.clone()).unwrap_or_default()
    }

    /// The server took it: the row says so for a moment, then goes; the shell's call is answered.
    pub(crate) fn went(&mut self, id: &str, now: Millis, out: &mut Out) -> Option<Asker> {
        let e = self.entries.get_mut(id)?;
        e.status = PendingStatus::Sent;
        e.due = Some(now.saturating_add(SENT_SHOWN));
        let asker = e.asker.take();
        self.show(id, out);
        asker
    }

    /// The session went before an answer came: the row waits for the next one, with the same request.
    pub(crate) fn wait(&mut self, id: &str, out: &mut Out) {
        if let Some(e) = self.entries.get_mut(id) {
            e.status = PendingStatus::Waiting;
            e.due = None;
            self.show(id, out);
        }
    }

    /// It did not go (`failure`): the row says why and waits for the owner. `answered`: the server answered its request,
    /// so trying it again is a new action. The shell's call is answered with the failure.
    pub(crate) fn failed(&mut self, id: &str, failure: &Failure, answered: bool, out: &mut Out) -> Option<Asker> {
        let e = self.entries.get_mut(id)?;
        e.status = PendingStatus::Failed;
        e.why.clone_from(&failure.message);
        e.due = None;
        e.answered |= answered;
        let asker = e.asker.take();
        self.show(id, out);
        asker
    }

    /// The owner tries a failed row again: it waits to go, with a new request when the server answered the last.
    pub(crate) fn resend(&mut self, calls: &mut Calls, id: &str, out: &mut Out) {
        let Some(e) = self.entries.get_mut(id) else { return };
        if e.status != PendingStatus::Failed {
            return;
        }
        if e.answered {
            e.request = calls.request();
            e.answered = false;
        }
        e.status = PendingStatus::Waiting;
        e.why.clear();
        self.show(id, out);
    }

    /// The owner takes back a row that is not on its way: it goes, and its drafts show in their box again. The shell's
    /// call, when not answered yet, is answered with why.
    pub(crate) fn unsend(&mut self, id: &str, out: &mut Out) -> Option<Option<Asker>> {
        if !self
            .entries
            .get(id)
            .is_some_and(|e| matches!(e.status, PendingStatus::Waiting | PendingStatus::Failed))
        {
            return None;
        }
        let entry = self.entries.remove(id)?;
        self.hide(id, out);
        Some(entry.asker)
    }

    /// Rows asked a while ago and not answered say so; rows that said they were sent go.
    pub(crate) fn tick(&mut self, now: Millis, out: &mut Out) {
        let due: Vec<Key> = self
            .entries
            .iter()
            .filter(|(_, e)| e.due.is_some_and(|at| at <= now))
            .map(|(id, _)| id.clone())
            .collect();
        for id in due {
            let Some(e) = self.entries.get_mut(&id) else { continue };
            e.due = None;
            if e.status == PendingStatus::Sent {
                self.entries.remove(&id);
                self.hide(&id, out);
            } else {
                e.status = PendingStatus::Slow;
                self.show(&id, out);
            }
        }
    }

    pub(crate) fn next_deadline(&self) -> Option<Millis> {
        self.entries.values().filter_map(|e| e.due).min()
    }

    /// What to keep across a reload, once a row changed since it was last said: every send not taken yet, oldest first,
    /// with what was typed in its drafts that the server does not have yet.
    /// A tab joined: every tab is told what to keep at the end of this step, the new one with the rest.
    pub(crate) fn keep_again(&mut self) {
        self.changed = true;
    }

    pub(crate) fn kept(&mut self, drafts: &Drafts) -> Option<Kept> {
        if !std::mem::take(&mut self.changed) {
            return None;
        }
        let sends = self
            .shown
            .keys()
            .filter_map(|(id, ())| self.entries.get(id).map(|e| (id, e)))
            .filter(|(_, e)| e.status != PendingStatus::Sent)
            .map(|(id, e)| KeptSend {
                row: self.row(id, e),
                call: e.call.clone(),
                request: e.request.id.clone(),
                answered: e.answered,
                typed: e.ids.iter().filter_map(|d| drafts.kept(d)).collect(),
            })
            .collect();
        Some(Kept { sends })
    }

    /// The sends an earlier page kept: each waits to go again with its request (a failed one waits for the owner), and
    /// what was typed in its drafts is held to be saved first.
    pub(crate) fn restore(&mut self, kept: Kept, now: Millis, drafts: &mut Drafts, out: &mut Out) {
        for send in kept.sends {
            let KeptSend {
                row,
                call,
                request,
                answered,
                typed,
            } = send;
            if self.entries.contains_key(&row.id) {
                continue;
            }
            for t in typed {
                drafts.restore(now, t, out);
            }
            let status = if row.status == PendingStatus::Failed {
                PendingStatus::Failed
            } else {
                PendingStatus::Waiting
            };
            self.entries.insert(
                row.id.clone(),
                Entry {
                    asker: None,
                    ids: row.drafts,
                    call,
                    at: row.at,
                    text: row.text,
                    request: Request { id: request },
                    answered,
                    status,
                    why: row.why,
                    due: None,
                },
            );
            self.show(&row.id, out);
        }
    }

    fn hide(&mut self, id: &str, out: &mut Out) {
        self.changed = true;
        self.shown.remove(id, &mut out.patches);
    }

    fn row(&self, id: &str, e: &Entry) -> Pending {
        Pending {
            id: id.to_owned(),
            at: e.at.clone(),
            text: e.text.clone(),
            drafts: e.ids.clone(),
            status: e.status,
            why: e.why.clone(),
        }
    }

    fn show(&mut self, id: &str, out: &mut Out) {
        let Some(e) = self.entries.get(id) else { return };
        let row = self.row(id, e);
        self.changed = true;
        // A row of text and an enum always encodes.
        if let Ok(record) = serde_json::to_value(&row) {
            self.shown.put(id.to_owned(), (), record, None, &mut out.patches);
        }
    }
}
