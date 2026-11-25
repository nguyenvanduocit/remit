//! A recap is what a card did so far, in words, for an agent that does not remember it: the Assistant's new session, or
//! another agent reading a card. What it carries is bounded: the newest of what happened, each item cut short, up to
//! `RECAP_HISTORY_RUNES`.

use std::fmt::Write as _;

use model::{Event, EventKind};

use crate::outbox::PRIORITY_OPENING;

const RECAP_HISTORY_RUNES: usize = 8000;
const RECAP_ITEM_RUNES: usize = 300;
/// The newest events of the card a recap is made from.
pub const RECAP_EVENTS: i64 = 400;

/// What a card was sent, said and ran, from history (its events, oldest first), apart from the openings of its sessions,
/// which say who it is rather than what happened: oldest first, each item cut short, the newest kept within
/// `RECAP_HISTORY_RUNES` after a line saying how many earlier ones are left out. Empty when nothing happened yet.
pub fn what_happened(history: &[Event]) -> String {
    let mut items: Vec<String> = Vec::new();
    for e in history {
        match e.kind() {
            EventKind::Prompt => {
                if let Some(by) = e.by.as_ref().filter(|_| e.priority != PRIORITY_OPENING) {
                    items.push(format!("Message from {}: {}", by.kind, clip(&e.text, RECAP_ITEM_RUNES)));
                }
            }
            EventKind::Message => items.push(format!("You said: {}", clip(&e.text, RECAP_ITEM_RUNES))),
            EventKind::Tool => items.push(format!("You ran: {} ({})", clip(&e.title, RECAP_ITEM_RUNES), e.status)),
            _ => {}
        }
    }
    if items.is_empty() {
        return String::new();
    }
    // What is kept is the newest.
    let (mut kept, mut size) = (0_usize, 0_usize);
    for item in items.iter().rev() {
        let n = item.chars().count().saturating_add(1);
        if kept > 0 && size.saturating_add(n) > RECAP_HISTORY_RUNES {
            break;
        }
        size = size.saturating_add(n);
        kept = kept.saturating_add(1);
    }
    let left = items.len().saturating_sub(kept);
    let mut b = String::new();
    if left > 0 {
        // Writing to a String does not fail.
        writeln!(b, "({left} earlier events are left out)").ok();
    }
    b.push_str(&items.get(left..).unwrap_or_default().join("\n"));
    b
}

/// s trimmed and cut to at most n characters, with an ellipsis when it was cut.
pub fn clip(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.chars().count() > n {
        let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
        out.push('…');
        return out;
    }
    s.to_owned()
}
