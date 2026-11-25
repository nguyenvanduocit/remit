//! The owner's unsent words (`drafts.ts`). The server keeps them; the replica of their home holds them, kept live by
//! pushes. Held here is only what the owner typed that the server does not have yet, laid over the server's copy until
//! its save is answered (the `Typed` store). Typing saves after a pause; a draft's saves and its deletion go one after the
//! other, so they reach the server in the order they were made. Each save is made on the revision of the server's copy
//! the words were typed over: when another page has saved since, the server refuses it with the draft as it is, and the
//! words go on from that copy with nothing of either page's lost (`rebase`). A send first saves what was typed, then sends
//! the drafts by their ids: what goes is what the server has, and the server ends exactly those drafts.

use std::collections::{HashMap, HashSet, VecDeque};

use link::{ErrorCode, Failure, Millis};
use model::{AttachmentRef, Draft, FailureDetail};
use serde_json::Value;
use wire::app::{DraftInput, client};

use crate::assistant::Assistant;
use crate::calls::{Calls, Purpose};
use crate::held::HeldChains;
use crate::index::Index;
use crate::records::{Out, read};
use crate::shell::{KeptDraft, Key, Model, Notice, What};

/// How long without typing before the words are saved.
pub(crate) const PAUSE: Millis = 400;
/// How long before a save that could not go is tried again.
pub(crate) const RETRY: Millis = 2_000;

/// The replicas a draft's home may be (the chains held, the Assistant): all are told, and each ignores what is not its
/// own.
pub(crate) struct Homes<'a> {
    pub(crate) chains: &'a mut HeldChains,
    pub(crate) assistant: &'a mut Assistant,
}

impl Homes<'_> {
    fn put(&mut self, d: &Draft, record: Value, out: &mut Out) {
        self.chains.put_draft(&d.home, d.id.clone(), d.rev, record.clone(), out);
        self.assistant.put_draft(&d.home, d.id.clone(), d.rev, record, out);
    }

    /// The revision of the server's copy of the draft that a replica holds; 0 (none) when neither does.
    fn rev(&self, id: &str) -> i64 {
        self.chains
            .draft_rev(id)
            .or_else(|| self.assistant.draft_rev(id))
            .unwrap_or(0)
    }

    /// Drops the draft from whichever replica holds it: a draft's id is its own, so no home is needed to find it, and
    /// a draft already saved has no typed copy to name one.
    fn drop(&mut self, id: &str, out: &mut Out) {
        self.chains.drop_draft(id, out);
        self.assistant.drop_draft(id, out);
    }
}

#[derive(Debug)]
struct Typed {
    draft: DraftInput,
    json: Value,
    /// Which writing this is: a save answered for an older one leaves the newer words held.
    version: u64,
    /// The revision of the server's copy the words were typed over (0: none), which their save is made on.
    base_rev: i64,
}

/// A draft's save under way: which writing of which draft, the send waiting for it, and whether it deletes a draft
/// left blank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Saving {
    id: Key,
    version: u64,
    send: Option<u64>,
    deleted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    /// A save; `send` names the send waiting for it.
    Save { send: Option<u64> },
    /// The owner discarded the draft: the server deletes it.
    Delete,
}

#[derive(Debug, Default)]
struct Turn {
    busy: bool,
    ops: VecDeque<Op>,
}

#[derive(Debug)]
struct Waiting {
    /// The outbox's row of the send.
    send: Key,
    left: HashSet<Key>,
    saved: bool,
}

/// A send whose drafts' saves are all answered: the core asks it (`saved`), or says they are not saved yet.
#[derive(Debug)]
pub(crate) struct Ready {
    /// The outbox's row of the send.
    pub(crate) send: Key,
    pub(crate) saved: bool,
}

#[derive(Debug)]
pub(crate) struct Drafts {
    typed: HashMap<Key, Typed>,
    shown: Index<()>,
    timers: HashMap<Key, Millis>,
    turns: HashMap<Key, Turn>,
    /// Drafts being sent: not saved meanwhile, the send ends them.
    sending: HashSet<Key>,
    waiting: HashMap<u64, Waiting>,
    pub(crate) ready: Vec<Ready>,
    next_version: u64,
    next_send: u64,
}

impl Default for Drafts {
    fn default() -> Self {
        Self {
            typed: HashMap::new(),
            shown: Index::new(Model::Typed),
            timers: HashMap::new(),
            turns: HashMap::new(),
            sending: HashSet::new(),
            waiting: HashMap::new(),
            ready: Vec::new(),
            next_version: 0,
            next_send: 0,
        }
    }
}

/// A draft's id, set in both its readings.
fn with_id(mut draft: DraftInput, mut json: Value, id: &str) -> (DraftInput, Value) {
    draft.id = id.to_owned();
    if let Some(fields) = json.as_object_mut() {
        fields.insert("id".to_owned(), Value::String(id.to_owned()));
    }
    (draft, json)
}

impl Drafts {
    /// When the next save is due.
    pub(crate) fn next_deadline(&self) -> Option<Millis> {
        self.timers.values().min().copied()
    }

    /// What the owner wrote in a draft (`{id: ""}` for a new one): shown at once, saved after a pause, on the copy it
    /// was typed over: the one its earlier words were, else the server's the page holds.
    pub(crate) fn write(
        &mut self,
        calls: &mut Calls,
        homes: &Homes<'_>,
        json: Value,
        out: &mut Out,
    ) -> Result<(), Failure> {
        let draft: DraftInput = read(&json)?;
        let id = if draft.id.is_empty() {
            calls.request().id
        } else {
            draft.id.clone()
        };
        let base_rev = self.typed.get(&id).map_or_else(|| homes.rev(&id), |t| t.base_rev);
        let (draft, json) = with_id(draft, json, &id);
        self.hold(calls.now, draft, json, base_rev, out);
        Ok(())
    }

    /// What was typed in a draft that the server does not have yet, to keep across a reload.
    pub(crate) fn kept(&self, id: &str) -> Option<KeptDraft> {
        self.typed.get(id).map(|t| KeptDraft {
            draft: t.json.clone(),
            base_rev: t.base_rev,
        })
    }

    /// What an earlier page kept of a draft's words: held, and saved after a pause (a send saves it first).
    pub(crate) fn restore(&mut self, now: Millis, mut kept: KeptDraft, out: &mut Out) {
        // A send kept by the image-only composer still names the same uploads after migration 0131.
        if let Some(fields) = kept.draft.as_object_mut()
            && let Some(images) = fields.remove("images")
        {
            fields.entry("attachments").or_insert(images);
        }
        // A kept draft this build cannot read is dropped: its send then fails, and says so, with its words in its row.
        if let Ok(draft) = read::<DraftInput>(&kept.draft) {
            self.hold(now, draft, kept.draft, kept.base_rev, out);
        }
    }

    fn hold(&mut self, now: Millis, draft: DraftInput, json: Value, base_rev: i64, out: &mut Out) {
        let id = draft.id.clone();
        self.next_version = self.next_version.wrapping_add(1);
        self.shown.put(id.clone(), (), json.clone(), None, &mut out.patches);
        self.typed.insert(
            id.clone(),
            Typed {
                draft,
                json,
                version: self.next_version,
                base_rev,
            },
        );
        self.timers.insert(id, now.saturating_add(PAUSE));
    }

    fn forget_typed(&mut self, id: &str, out: &mut Out) -> Option<Typed> {
        let typed = self.typed.remove(id)?;
        self.shown.remove(id, &mut out.patches);
        Some(typed)
    }

    /// Saves the drafts whose pause is over. While the outbox `holds` a send, a box's draft the server does not have yet
    /// waits: the draft being sent may still be in that box, which takes one draft, and the server would answer the
    /// first save with it (`rebase`), joining the new words into the message on its way.
    pub(crate) fn tick(&mut self, calls: &mut Calls, holds: bool, out: &mut Out) {
        let now = calls.now;
        let due: Vec<Key> = self
            .timers
            .iter()
            .filter(|(_, at)| **at <= now)
            .map(|(id, _)| id.clone())
            .collect();
        for id in due {
            let waits = holds && self.typed.get(&id).is_some_and(|t| t.base_rev == 0);
            if waits {
                self.timers.insert(id, now.saturating_add(RETRY));
                continue;
            }
            self.timers.remove(&id);
            self.queue(calls, &id, Op::Save { send: None }, out);
        }
    }

    /// The owner no longer wants the draft: it goes from every page, and the server deletes it after what is in turn.
    pub(crate) fn discard(&mut self, calls: &mut Calls, homes: &mut Homes<'_>, id: &str, out: &mut Out) {
        self.timers.remove(id);
        self.forget_typed(id, out);
        homes.drop(id, out);
        self.queue(calls, id, Op::Delete, out);
    }

    /// Readies the outbox's `send` of these drafts once each has saved what was typed in it.
    pub(crate) fn send(&mut self, calls: &mut Calls, send: Key, ids: &[Key], out: &mut Out) {
        let token = self.next_send;
        self.next_send = self.next_send.wrapping_add(1);
        let left: HashSet<Key> = ids.iter().cloned().collect();
        self.waiting.insert(
            token,
            Waiting {
                send,
                left,
                saved: true,
            },
        );
        self.settle_waiting(token);
        for id in ids {
            self.timers.remove(id);
            self.queue(calls, id, Op::Save { send: Some(token) }, out);
        }
    }

    /// The send was asked: its drafts are held off saving until it is answered.
    pub(crate) fn sending(&mut self, ids: &[Key]) {
        self.sending.extend(ids.iter().cloned());
    }

    /// The send was answered: the drafts that went leave the page; what was typed in them meanwhile is saved.
    pub(crate) fn sent(&mut self, calls: &mut Calls, homes: &mut Homes<'_>, ids: &[Key], went: bool, out: &mut Out) {
        for id in ids {
            if went {
                self.ended(calls, homes, id, out);
            }
            self.sending.remove(id);
            if self.typed.contains_key(id) {
                self.timers.insert(id.clone(), calls.now.saturating_add(PAUSE));
            }
        }
    }

    fn queue(&mut self, calls: &mut Calls, id: &str, op: Op, out: &mut Out) {
        self.turns.entry(id.to_owned()).or_default().ops.push_back(op);
        self.pump(calls, id, out);
    }

    /// Runs the draft's next op once the one before it was answered.
    fn pump(&mut self, calls: &mut Calls, id: &str, out: &mut Out) {
        loop {
            let Some(turn) = self.turns.get_mut(id) else { return };
            if turn.busy {
                return;
            }
            let Some(op) = turn.ops.pop_front() else {
                self.turns.remove(id);
                return;
            };
            let asked = match op {
                Op::Save { send } => match self.start_save(calls, id, send) {
                    Some(saved) => {
                        self.saved(send, id, saved);
                        false
                    }
                    None => true,
                },
                Op::Delete => {
                    if calls.live() {
                        let request = calls.request();
                        let purpose = Purpose::Discard { id: id.to_owned() };
                        calls.ask(client::draft_delete(request, id.to_owned()), purpose);
                        true
                    } else {
                        out.notice(Notice::CouldNot {
                            what: What::DeleteDraft,
                            failure: Failure::new(ErrorCode::Disconnected, crate::core::OFFLINE),
                        });
                        false
                    }
                }
            };
            if asked && let Some(turn) = self.turns.get_mut(id) {
                turn.busy = true;
            }
        }
    }

    /// Asks the server to save what was typed; whether it has it already, when nothing was asked.
    fn start_save(&mut self, calls: &mut Calls, id: &str, send: Option<u64>) -> Option<bool> {
        let Some(typed) = self.typed.get(id) else {
            return Some(true);
        };
        if self.sending.contains(id) {
            // The send ends it; what is typed meanwhile becomes a new draft once it has.
            return Some(true);
        }
        let blank = typed.draft.text.trim().is_empty() && typed.draft.attachments.is_empty();
        if !calls.live() {
            self.timers.insert(id.to_owned(), calls.now.saturating_add(RETRY));
            return Some(false);
        }
        let request = calls.request();
        let saving = Saving {
            id: id.to_owned(),
            version: typed.version,
            send,
            deleted: blank,
        };
        if blank {
            calls.ask(client::draft_delete(request, id.to_owned()), Purpose::DraftSave(saving));
        } else {
            calls.ask(
                client::draft_save(request, typed.draft.clone(), typed.base_rev),
                Purpose::DraftSave(saving),
            );
        }
        None
    }

    /// A save's answer (`deleted`: a blank draft's deletion).
    pub(crate) fn save_answered(
        &mut self,
        calls: &mut Calls,
        homes: &mut Homes<'_>,
        saving: Saving,
        result: Result<Value, Failure>,
        out: &mut Out,
    ) {
        let Saving {
            id,
            version,
            send,
            deleted,
        } = saving;
        let id = id.as_str();
        let saved = match result {
            Ok(_) if deleted => {
                if self.typed.contains_key(id) {
                    homes.drop(id, out);
                }
                self.settle_version(id, version, out);
                true
            }
            Ok(json) => match read::<Draft>(&json) {
                Ok(stored) => {
                    homes.put(&stored, json, out);
                    self.settle_version(id, version, out);
                    // words typed while it was saved go on from what it saved
                    if let Some(newer) = self.typed.get_mut(id) {
                        newer.base_rev = stored.rev;
                    }
                    true
                }
                Err(failure) => self.save_failed(calls, homes, id, failure, out),
            },
            Err(failure) => self.save_failed(calls, homes, id, failure, out),
        };
        self.saved(send, id, saved);
        self.next(calls, id, out);
    }

    /// The deletion the owner's discard asked for was answered.
    pub(crate) fn discard_answered(
        &mut self,
        calls: &mut Calls,
        id: &str,
        result: Result<Value, Failure>,
        out: &mut Out,
    ) {
        if let Err(failure) = result {
            out.notice(Notice::CouldNot {
                what: What::DeleteDraft,
                failure,
            });
        }
        self.next(calls, id, out);
    }

    fn next(&mut self, calls: &mut Calls, id: &str, out: &mut Out) {
        if let Some(turn) = self.turns.get_mut(id) {
            turn.busy = false;
        }
        self.pump(calls, id, out);
    }

    /// The words saved are what is held: they need not be laid over the server's copy any longer.
    fn settle_version(&mut self, id: &str, version: u64, out: &mut Out) {
        if self.typed.get(id).is_some_and(|t| t.version == version) {
            self.forget_typed(id, out);
        }
    }

    /// The server refused words made on a copy another page has saved over since (or a page's first in a box that
    /// holds a draft already), and sent the draft as it is: it is shown, and what is typed goes on from it, keeping the
    /// words of both pages (`joined`), saved after a pause.
    fn rebase(&mut self, now: Millis, homes: &mut Homes<'_>, id: &str, current: Draft, out: &mut Out) {
        if let Ok(record) = serde_json::to_value(&current) {
            homes.put(&current, record, out);
        }
        self.timers.remove(id);
        let Some(typed) = self.forget_typed(id, out) else {
            return;
        };
        let text = joined(&current.text, &typed.draft.text);
        let attachments = joined_attachments(&current.attachments, &typed.draft.attachments);
        let (mut draft, mut json) = with_id(typed.draft, typed.json, &current.id);
        draft.text.clone_from(&text);
        draft.attachments.clone_from(&attachments);
        if let Some(fields) = json.as_object_mut() {
            fields.insert("text".to_owned(), Value::String(text));
            fields.insert("attachments".to_owned(), serde_json::json!(attachments));
        }
        self.hold(now, draft, json, current.rev, out);
    }

    fn save_failed(
        &mut self,
        calls: &mut Calls,
        homes: &mut Homes<'_>,
        id: &str,
        failure: Failure,
        out: &mut Out,
    ) -> bool {
        if failure.code == ErrorCode::Ended {
            // Sent or deleted, here or on another page.
            self.ended(calls, homes, id, out);
        } else if let Some(FailureDetail::DraftConflict { current }) = failure.detail_as() {
            self.rebase(calls.now, homes, id, current, out);
        } else if !calls.live() {
            self.timers.insert(id.to_owned(), calls.now.saturating_add(RETRY));
        } else {
            out.notice(Notice::CouldNot {
                what: What::SaveDraft,
                failure,
            });
        }
        false
    }

    /// The server ended the draft (sent, or deleted): it goes from the page, and words typed in it since stay, as a new
    /// draft in the same place.
    fn ended(&mut self, calls: &mut Calls, homes: &mut Homes<'_>, id: &str, out: &mut Out) {
        self.timers.remove(id);
        let left = self.forget_typed(id, out);
        homes.drop(id, out);
        if let Some(left) = left
            && !left.draft.text.trim().is_empty()
        {
            let fresh = calls.request().id;
            let (draft, json) = with_id(left.draft, left.json, &fresh);
            self.hold(calls.now, draft, json, 0, out);
        }
    }

    /// A save a send waited for is answered.
    fn saved(&mut self, send: Option<u64>, id: &str, saved: bool) {
        let Some(token) = send else { return };
        if let Some(w) = self.waiting.get_mut(&token) {
            w.left.remove(id);
            w.saved &= saved;
        }
        self.settle_waiting(token);
    }

    fn settle_waiting(&mut self, token: u64) {
        if self.waiting.get(&token).is_some_and(|w| w.left.is_empty())
            && let Some(w) = self.waiting.remove(&token)
        {
            self.ready.push(Ready {
                send: w.send,
                saved: w.saved,
            });
        }
    }
}

/// The words of a draft two pages wrote over one copy, with nothing of either lost: the one that holds the other already
/// (the same words, one page ahead), else the server's, then this page's under them.
fn joined(server: &str, typed: &str) -> String {
    if typed.contains(server.trim()) {
        typed.to_owned()
    } else if server.contains(typed.trim()) {
        server.to_owned()
    } else {
        format!("{}\n\n{}", server.trim_end(), typed.trim_start())
    }
}

/// The attachments of a draft two pages wrote: the server's, then those typed here that it lacks.
fn joined_attachments(server: &[AttachmentRef], typed: &[AttachmentRef]) -> Vec<AttachmentRef> {
    let mut out = server.to_vec();
    out.extend(typed.iter().filter(|t| !server.iter().any(|s| s.id == t.id)).cloned());
    out
}
