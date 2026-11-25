//! A list of an agent's events (the open chain's, one card's, the Assistant's), oldest first: an event
//! that changes is put unless the copy held is newer, and a streamed chunk grows the event it belongs to.

use serde_json::Value;
use wire::app::{EventDelta, EventRead};

use crate::index::{Index, newer};
use crate::models::Patch;
use crate::records::js_len;
use crate::shell::Key;

#[derive(Debug, Clone, Copy)]
pub(crate) struct EventFacts {
    pub(crate) id: i64,
    rev: i64,
    /// The length of its text, in UTF-16 code units: where the next chunk starts.
    len: usize,
}

impl EventFacts {
    pub(crate) fn of(read: &EventRead) -> Self {
        let e = &read.event;
        Self {
            id: e.id,
            rev: e.rev,
            len: js_len(&e.text),
        }
    }
}

pub(crate) fn key(id: i64) -> Key {
    id.to_string()
}

pub(crate) fn reset(ix: &mut Index<EventFacts>, events: Vec<(EventRead, Value)>, out: &mut Vec<Patch>) {
    let rows = events
        .into_iter()
        .map(|(e, v)| (key(e.event.id), EventFacts::of(&e), v))
        .collect();
    ix.reset(rows, out);
}

/// Puts an event unless the copy held is newer. Within one revision an event's text only grows as it streams, and the
/// server sends a copy with the text streamed since merged into it (`rpc` `pending.rs`): of two copies of one revision,
/// the longer is the newer.
pub(crate) fn put(ix: &mut Index<EventFacts>, e: &EventRead, record: Value, out: &mut Vec<Patch>) {
    let id = key(e.event.id);
    let arrived = EventFacts::of(e);
    let take = ix
        .get(&id)
        .is_none_or(|held| newer(Some(held.rev), arrived.rev) || (held.rev == arrived.rev && arrived.len > held.len));
    if take {
        ix.put(id, arrived, record, None, out);
    }
}

/// Grows the event by what a chunk carries past the held text. The server merges the chunks a session has not been sent
/// yet into one (`rpc` `pending.rs`), so a chunk can start inside text the core read meanwhile: only its rest is new. A
/// chunk that starts past the held text comes after one missed: it is left, and the event's next update brings the
/// whole text.
pub(crate) fn grow(ix: &mut Index<EventFacts>, d: &EventDelta, out: &mut Vec<Patch>) {
    let id = key(d.id);
    let slot = ix.slot().clone();
    let Some(facts) = ix.meta_mut(&id) else { return };
    let Some(text) = facts
        .len
        .checked_sub(d.offset)
        .and_then(|held| past(&d.text, held))
        .filter(|rest| !rest.is_empty())
    else {
        return;
    };
    facts.len = facts.len.saturating_add(js_len(text));
    out.push(Patch::Append {
        slot,
        id,
        text: text.to_owned(),
    });
}

/// What of text comes after its first `units` UTF-16 code units; none when that cut is not at a character's edge.
fn past(text: &str, units: usize) -> Option<&str> {
    let mut left = units;
    for (at, c) in text.char_indices() {
        if left == 0 {
            return text.get(at..);
        }
        left = left.checked_sub(c.len_utf16())?;
    }
    (left == 0).then_some("")
}
