//! The repetition rail (ADR-0092): an agent that keeps calling the same tool for the same answer, or keeps meeting the same
//! error, is in a loop the stall and silent-turn rails miss, since it keeps sending. Each finished tool call of a turn is
//! fingerprinted with what changes from call to call masked (every word with a digit: times, ids, counters, sizes), and
//! the last `WINDOW` are kept. A repeat steers a note into the turn; a second asks the agent to end the turn with its
//! report; calls past that end it. Deterministic, per run, in memory: `Repeats::saw` is the one decision, and deleting
//! this file and its two callers removes the rail.

use std::collections::VecDeque;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use crate::chain_core::Core;
use crate::outbox::by_remit;
use crate::session::Run;
use model::Cut;

/// The finished calls of a turn that are compared, newest last: enough to see a cycle of a few calls come round.
const WINDOW: usize = 8;

/// The calls an agent told to end its turn may still make (its plan, its report) before Remit ends the turn.
const END_GRACE: usize = 3;

/// What the rail does after a finished call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Repetition {
    /// Nothing: the work goes on.
    Go,
    /// Steer this note into the turn: the agent repeats itself.
    Note(String),
    /// Steer this note into the turn: the agent repeats itself again, and is to end the turn with its report.
    End(String),
    /// End the turn: told to end it, the agent kept calling tools.
    Cut,
}

/// One finished call: its id, the print of the call with its result, and the print of its error when it failed; the
/// prints are dropped once a sign was given, so only newer calls count.
#[derive(Debug, Clone)]
struct Step {
    id: String,
    call: Option<u64>,
    error: Option<u64>,
}

/// What the rail knows of the turn a run is in: its finished calls since the last sign, the signs given, and the calls
/// made since the agent was told to end the turn.
#[derive(Debug, Default)]
pub struct Repeats {
    steps: VecDeque<Step>,
    signs: u8,
    after_end: usize,
}

impl Repeats {
    /// Takes call t as the agent's process last said it, most being how many times the same call with the same result, or
    /// the same error, makes a sign (0: the rail is off), and says what the rail does. A call counts once, when it has
    /// finished; after a sign only newer calls count, so a long poll is judged afresh and not stopped by its count.
    pub fn saw(&mut self, t: &wire::runtime::ToolCall, most: usize) -> Repetition {
        let finished = t.status.ended();
        if most == 0 || !finished || self.steps.iter().any(|s| s.id == t.id) {
            return Repetition::Go;
        }
        let step = Step {
            id: t.id.clone(),
            call: Some(print(&[
                &t.name,
                &serde_json::to_string(&t.input).unwrap_or_default(),
                &t.output,
            ])),
            error: (t.status == model::ToolStatus::Failed).then(|| print(&[&t.output])),
        };
        let like =
            |same: fn(&Step, &Step) -> bool| self.steps.iter().filter(|s| same(s, &step)).count().saturating_add(1);
        let (calls, errors) = (
            like(|a, b| a.call.is_some() && a.call == b.call),
            like(|a, b| a.error.is_some() && a.error == b.error),
        );
        let failed = step.error.is_some();
        self.steps.push_back(step);
        if self.steps.len() > WINDOW {
            self.steps.pop_front();
        }
        if self.signs >= 2 {
            self.after_end = self.after_end.saturating_add(1);
            return if self.after_end == END_GRACE {
                Repetition::Cut
            } else {
                Repetition::Go
            };
        }
        let what = if calls >= most {
            format!("You ran {} {calls} times, with the same result", cut_to(&t.title, 160))
        } else if failed && errors >= most {
            format!(
                "You met the same error {errors} times: {}",
                cut_to(first_line(&t.output), 160)
            )
        } else {
            return Repetition::Go;
        };
        // only calls after the sign count from here; the ids stay, so a call said again is still known
        for s in &mut self.steps {
            (s.call, s.error) = (None, None);
        }
        self.signs = self.signs.saturating_add(1);
        if self.signs == 1 {
            Repetition::Note(format!(
                "[Remit: you are repeating yourself] {what}. Doing it again will not change what comes back. Try \
                 another way, or say plainly what blocks you."
            ))
        } else {
            Repetition::End(format!(
                "[Remit: you are still repeating yourself] {what}. Make no more tool calls: end this turn now with your \
                 report, saying what you did, what you tried and what is left. Remit ends the turn if the calls go on."
            ))
        }
    }
}

/// A print of parts, with every word that holds a digit masked and spacing collapsed, so the same call made at another
/// time, on another id or with another counter prints the same.
fn print(parts: &[&str]) -> u64 {
    let mut h = DefaultHasher::new();
    for p in parts {
        masked(p).hash(&mut h);
    }
    h.finish()
}

/// s with every run of letters and digits that holds a digit written `#`, and every run of spacing one space.
fn masked(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut word = String::new();
    let mut space = false;
    let flush = |word: &mut String, out: &mut String| {
        if word.chars().any(|c| c.is_ascii_digit()) {
            out.push('#');
        } else {
            out.push_str(word);
        }
        word.clear();
    };
    for c in s.chars() {
        if c.is_alphanumeric() {
            word.push(c);
            continue;
        }
        flush(&mut word, &mut out);
        if c.is_whitespace() {
            space = true;
            continue;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(c);
    }
    flush(&mut word, &mut out);
    out
}

fn first_line(s: &str) -> &str {
    s.lines().find(|l| !l.trim().is_empty()).unwrap_or_default().trim()
}

/// s cut to at most n characters, on a whole character.
fn cut_to(s: &str, n: usize) -> String {
    match s.char_indices().nth(n) {
        Some((at, _)) => format!("{}…", &s[..at]),
        None => s.to_owned(),
    }
}

impl Core {
    /// What the repetition rail does about call t of r's turn, as its process last said it (`Repeats::saw`). A subagent's
    /// run is its parent's to hold.
    pub fn repeats_of(&self, r: &Run, t: &wire::runtime::ToolCall) -> Repetition {
        if r.root.is_some() {
            return Repetition::Go;
        }
        r.state().repeats.saw(t, self.cfg.turn_repeats)
    }

    /// Acts on repetition in r's turn, once the call it is about is recorded: the note goes into the turn, as a message from
    /// Remit the stream shows, and a turn the agent was told to end and did not is cut short. A turn that ended meanwhile
    /// is left alone.
    pub async fn repeat_rail(self: &Arc<Self>, r: &Arc<Run>, repetition: Repetition) {
        if repetition == Repetition::Go || !r.busy() {
            return;
        }
        let card = r.card_id();
        let (title, note) = match repetition {
            Repetition::Go => return,
            Repetition::Cut => {
                tracing::info!(home = %r.home, %card, "repetition rail: the turn ends");
                self.cut_turn_of(r, &Cut::Repeating).await;
                return;
            }
            Repetition::Note(note) => ("You are repeating yourself", note),
            Repetition::End(note) => ("End the turn with your report", note),
        };
        tracing::info!(home = %r.home, %card, %title, "repetition rail");
        if let Err(err) = self.deliver_now(&card, title, &note, by_remit()).await {
            tracing::warn!(home = %r.home, %card, %err, "repetition rail: steer the note");
        }
    }
}
