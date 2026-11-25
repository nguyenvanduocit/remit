use std::collections::HashMap;
use std::sync::Mutex;

/// Bounds what the gate remembers of refusals: a program that tries ten thousand hosts must not grow the server's
/// memory, and an answer that lists them all would not be read. Targets past the bound are counted, not listed.
pub(crate) const MAX_DENIALS: usize = 64;

/// Bounds a refused target as shown: what a program wrote as a host is untrusted text.
pub(crate) const MAX_TARGET_TEXT: usize = 256;

/// One thing the gate refused, counted by how many times it was tried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Denial {
    /// The host:port the program asked for, as text safe to show.
    pub target: String,
    /// Which rule refused it.
    pub reason: String,
    /// How many requests were refused for this target and reason.
    pub count: usize,
}

/// The gate's record of refusals: at most `MAX_DENIALS` distinct (target, reason) pairs, then a count of the rest.
#[derive(Debug, Default)]
pub(crate) struct Denials {
    inner: Mutex<Record>,
}

#[derive(Debug, Default)]
struct Record {
    list: Vec<Denial>,
    index: HashMap<(String, String), usize>,
    overflow: usize,
}

impl Denials {
    pub(crate) fn add(&self, target: &str, reason: &str) {
        let key = (showable(target), reason.to_string());
        // A poisoned lock still holds a whole record: every change below is one step.
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let r = &mut *guard;
        // Counts saturate: a count past usize::MAX reads as "that many or more", never wraps to few.
        if let Some(d) = r.index.get(&key).and_then(|&i| r.list.get_mut(i)) {
            d.count = d.count.saturating_add(1);
            return;
        }
        if r.list.len() >= MAX_DENIALS {
            r.overflow = r.overflow.saturating_add(1);
            return;
        }
        let i = r.list.len();
        r.list.push(Denial {
            target: key.0.clone(),
            reason: key.1.clone(),
            count: 1,
        });
        r.index.insert(key, i);
    }

    /// The refusals in the order they first happened; when some were not listed, a last entry counts them.
    pub(crate) fn all(&self) -> Vec<Denial> {
        let r = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = r.list.clone();
        if r.overflow > 0 {
            out.push(Denial {
                target: "(other targets)".to_string(),
                reason: "more distinct refusals than the gate lists".to_string(),
                count: r.overflow,
            });
        }
        out
    }
}

/// Cuts a target to `MAX_TARGET_TEXT` bytes and replaces what is not printable ASCII with "?". A cut through a character
/// leaves bytes that are not UTF-8: they become "?" too, one per invalid sequence.
pub(crate) fn showable(s: &str) -> String {
    let bytes = s.as_bytes();
    let (cut, more) = match bytes.split_at_checked(MAX_TARGET_TEXT) {
        Some((cut, rest)) if !rest.is_empty() => (cut, true),
        _ => (bytes, false),
    };
    let mut out: String = String::from_utf8_lossy(cut)
        .chars()
        .map(|c| if (' '..='~').contains(&c) { c } else { '?' })
        .collect();
    if more {
        out.push_str("...");
    }
    out
}
