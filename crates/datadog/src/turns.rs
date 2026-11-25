//! The turns each card has run lately, as their `remit.turn` spans began, and where each trace a card's session exported
//! was put: the trace and span a session's own records go under. The server's layer writes a turn as its span is first
//! entered; the `/otlp` door places each trace once, by its earliest span, so every span and log of one trace stays in
//! one place whatever their clocks say or whichever batch carries them.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, PoisonError};

/// One turn of a card: when its span began, the message it answers, and the span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Turn {
    pub start_ns: u64,
    pub turn: i64,
    pub trace_id: [u8; 16],
    pub span_id: [u8; 8],
}

/// Where one trace a session exported goes: the turn's trace, or with no turn one of the card's own (`card_trace`), so
/// a payload's ids never reach into a trace that is not its card's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Placed {
    pub trace_id: [u8; 16],
    pub turn: Option<Turn>,
}

/// How many turns of one card are kept: an export comes within seconds of what it reports, so the last few cover it.
const PER_CARD: usize = 16;
/// How many traces of one card keep their place: a session opens one per interaction.
const TRACES_PER_CARD: usize = 256;
/// How many cards are kept: past it, the ones whose newest turn is oldest go.
const CARDS: usize = 4096;

#[derive(Debug, Default)]
struct Card {
    turns: VecDeque<Turn>,
    traces: VecDeque<([u8; 16], Placed)>,
}

impl Card {
    fn at(&self, at_ns: u64) -> Option<Turn> {
        self.turns.iter().rev().find(|t| t.start_ns <= at_ns).copied()
    }
}

#[derive(Debug, Default)]
pub struct Turns(Mutex<HashMap<String, Card>>);

impl Turns {
    /// Records that card's turn t began.
    pub(crate) fn began(&self, card: &str, t: Turn) {
        let mut m = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if m.len() >= CARDS && !m.contains_key(card) {
            let oldest = m
                .iter()
                .min_by_key(|(_, c)| c.turns.back().map_or(0, |t| t.start_ns))
                .map(|(c, _)| c.clone());
            if let Some(c) = oldest {
                m.remove(&c);
            }
        }
        let c = m.entry(card.to_owned()).or_default();
        if c.turns.len() >= PER_CARD {
            c.turns.pop_front();
        }
        c.turns.push_back(t);
    }

    /// The turn of card a record made at `at_ns` belongs to: the last that began at or before it. None before its first.
    pub fn at(&self, card: &str, at_ns: u64) -> Option<Turn> {
        let m = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        m.get(card)?.at(at_ns)
    }

    /// Where card's trace goes: where it went the first time, else in the turn its earliest span (`earliest_ns`) began in.
    pub(crate) fn place(&self, card: &str, trace: &[u8], earliest_ns: u64) -> Placed {
        let id: [u8; 16] = trace.try_into().unwrap_or_default();
        let mut m = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(c) = m.get_mut(card) else {
            return unplaced(card, id);
        };
        if let Some((_, p)) = c.traces.iter().find(|(t, _)| *t == id) {
            return *p;
        }
        let p = match c.at(earliest_ns) {
            Some(t) => Placed {
                trace_id: t.trace_id,
                turn: Some(t),
            },
            None => unplaced(card, id),
        };
        if c.traces.len() >= TRACES_PER_CARD {
            c.traces.pop_front();
        }
        c.traces.push_back((id, p));
        p
    }
}

fn unplaced(card: &str, trace: [u8; 16]) -> Placed {
    Placed {
        trace_id: card_trace(card, trace),
        turn: None,
    }
}

/// The card's own trace for a session's trace that fell in no turn: the same for every batch of it, and never another
/// card's, whatever ids the payload brings.
fn card_trace(card: &str, trace: [u8; 16]) -> [u8; 16] {
    let half = |n: u8| {
        let mut h = DefaultHasher::new();
        (card, trace, n).hash(&mut h);
        h.finish().to_be_bytes()
    };
    let mut id = [0; 16];
    for (d, s) in id.iter_mut().zip(half(0).into_iter().chain(half(1))) {
        *d = s;
    }
    id
}
