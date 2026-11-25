//! The no-progress rail (ADR-0104): a chain whose agents keep taking turns and produce nothing is reminded and then
//! finished, on the stall rail's path (schedule.rs `unstick`), and the Assistant's exchange with agents that goes on with
//! no word from the owner is reminded and then refused. The stall rail reads silence and the repetition rail one turn
//! (ADR-0092); this one reads what turns produce across turns. Pure but for `Core::replies_in_a_row`; deleting
//! this file and its callers removes the rail.

use model::{ActorKind, Card, Decision, DecisionKind, Event, EventStatus, PromptStatus};

use crate::chain_core::Core;
use crate::outbox::sent_by;
use crate::schedule::MAX_NUDGES;

/// The decisions that are something a chain produced: work accepted, a judge's verdict or pick, the goal checked, a change
/// landing or live, an insight told, a chain proposed, a release installed, a lesson shared. Hires, routes, reworks and
/// waits are how work is moved, not what it yields.
const PRODUCED: &[DecisionKind] = &[
    DecisionKind::Accept,
    DecisionKind::Verdict,
    DecisionKind::Duel,
    DecisionKind::Pick,
    DecisionKind::Goal,
    DecisionKind::Landing,
    DecisionKind::Live,
    DecisionKind::Insight,
    DecisionKind::Propose,
    DecisionKind::Release,
    DecisionKind::Learning,
];

/// What a chain's agents have done, as the rail counts it: the turns they took and what they produced (each step of a
/// plan completed, each decision in `PRODUCED`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tally {
    pub turns: i64,
    pub produced: i64,
}

/// A chain's tally from its cards and its decisions.
pub fn tally(cards: &[Card], decisions: &[Decision]) -> Tally {
    let steps = cards
        .iter()
        .flat_map(|c| &c.plan)
        .filter(|e| e.status == "completed")
        .count();
    let produced = decisions.iter().filter(|d| PRODUCED.contains(&d.kind)).count();
    Tally {
        turns: cards.iter().map(|c| c.turns).sum(),
        produced: i64::try_from(steps.saturating_add(produced)).unwrap_or(i64::MAX),
    }
}

/// How many reminders `idle` turns with nothing produced call for: one every `every` turns; none with the rail off
/// (`every` 0).
fn reminders(idle: i64, every: usize) -> i64 {
    match i64::try_from(every) {
        Ok(every) if every > 0 => idle.checked_div(every).unwrap_or_default(),
        _ => 0,
    }
}

/// A reminder the rail calls for: the how-many-th (past `MAX_NUDGES`: finish), and the turns taken since the chain
/// last produced something.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Due {
    pub n: i64,
    pub idle: i64,
}

/// What the rail keeps of a running chain, in memory: its tally when it last produced something, and the reminders
/// sent since. A chain that stops running is forgotten, so a wait or a pause starts the count again.
#[derive(Debug, Clone, Copy)]
pub struct Spin {
    at: Tally,
    told: i64,
}

impl Spin {
    /// The rail first seeing a chain whose tally is at.
    pub fn new(at: Tally) -> Self {
        Self { at, told: 0 }
    }

    /// Takes the chain's tally now, every being how many turns with nothing produced call for a reminder, and says
    /// which reminder is due, if one is. Something produced starts the count again; a plan rewritten with fewer steps
    /// completed is neither progress nor a loss.
    pub fn saw(&mut self, now: Tally, every: usize) -> Option<Due> {
        if now.produced > self.at.produced || now.turns < self.at.turns {
            *self = Self::new(now);
            return None;
        }
        self.at.produced = now.produced;
        let idle = now.turns.saturating_sub(self.at.turns);
        let due = reminders(idle, every);
        if due <= self.told {
            return None;
        }
        self.told = due;
        Some(Due { n: due, idle })
    }
}

/// What the Assistant's message to an agent meets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exchange {
    /// It goes.
    Go,
    /// It goes, and the Assistant reads this reminder with the tool's answer.
    Remind(String),
    /// It is refused, for this reason.
    Refuse(String),
}

/// What a message to an agent meets after the Assistant took replies turns in a row on agents' replies, every as for
/// a chain (`-spin-turns`).
pub fn exchange(replies: i64, every: usize) -> Exchange {
    match reminders(replies, every) {
        0 => Exchange::Go,
        n if n <= MAX_NUDGES => Exchange::Remind(format!(
            "[Remit: you took {replies} turns in a row on agents' replies, with no word from the owner and no item \
             between them (reminder {n} of {MAX_NUDGES})] An exchange that goes on without moving the owner's work is \
             a loop: end it unless this message moves the work. After the last reminder Remit sends no more messages \
             to agents until the owner writes or an item comes to you."
        )),
        _ => Exchange::Refuse(format!(
            "you took {replies} turns in a row on agents' replies with no word from the owner or an item between \
             them, after {MAX_NUDGES} reminders: Remit sends no more messages to agents until he writes or an item \
             comes to you. Tell him in your answer what the exchange was for and where it stands."
        )),
    }
}

/// Whether e, a message to the Assistant newest first, opened a turn on an agent's reply: `Some(true)`; one that
/// opened a turn on anything else (the owner's words, an item, Remit's): `Some(false)`; one that opened no turn (still
/// queued, or steered into a turn): `None`.
fn reply_turn(e: &Event) -> Option<bool> {
    (e.status == EventStatus::Prompt(PromptStatus::Delivered)).then(|| sent_by(e).kind == ActorKind::Agent)
}

impl Core {
    /// How many turns in a row, the one in progress included, the Assistant (card) took on agents' replies; counted no
    /// further than the rail needs. 0 when its messages cannot be read: the rail never stops the Assistant on a
    /// failed read.
    pub async fn replies_in_a_row(&self, card: &str) -> i64 {
        let card = card.to_owned();
        let every = i64::try_from(self.cfg.spin_turns).unwrap_or(i64::MAX);
        let enough = every.saturating_mul(MAX_NUDGES.saturating_add(1));
        let read = self
            .store
            .read(move |r| {
                let mut n = 0_i64;
                r.newest_prompt_before(&card, i64::MAX, |e| match reply_turn(e) {
                    Some(true) => {
                        n = n.saturating_add(1);
                        n >= enough
                    }
                    Some(false) => true,
                    None => false,
                })?;
                Ok::<_, store::Error>(n)
            })
            .await;
        read.unwrap_or_else(|err| {
            tracing::warn!(%err, "no-progress rail: read the Assistant's messages");
            0
        })
    }
}
