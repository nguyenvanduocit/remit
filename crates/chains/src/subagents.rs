//! Claude Code's subagents are standing employees at work (ADR-0073, decision 2). `subagent_types` says which types a
//! session is offered: every card keeps the built-ins general-purpose and fork, which run as the card's own employee; a
//! Chair and the Assistant are also offered each active worker within their ceiling, under its handle, built from its
//! loadout as a worker card is. A judge is offered none (`deny_for` denies the subagent tools to it). Each handle's type
//! reaches Remit through a server of its own, `remit_seat`, with a seat token: the employee at work for the session's
//! card, served the employee's own tools and the card's reads (`seat_server_for`), never the card's acts. The set and its
//! tokens are fixed when the session starts and end with it.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use model::{Card, Employee};

use crate::chain_core::Core;
use crate::chain_core::new_id;
use crate::session::Run;
use crate::spawn::Spawnee;
use crate::{Error, Result};

/// The built-in types every session that may spawn keeps: each runs as the spawning card's own employee.
pub const BUILTIN_SUBAGENTS: [&str; 2] = ["general-purpose", "fork"];

/// The name a subagent type's own Remit server goes by. It must differ from the session's (`MCP_SERVER_NAME`): a type's
/// server named as a parent's is replaced by the parent's, whose token would then speak for the subagent.
pub const SEAT_SERVER_NAME: &str = "remit_seat";

/// The one name an employee goes by as a subagent type, in both engines: its name, else the agent it was hired from, and
/// its number, lower case, as in ana-12. Claude Code takes letters, digits and hyphens.
pub fn employee_handle(e: &Employee) -> String {
    let word = |s: &str| {
        let mut b = String::new();
        for r in s.to_lowercase().chars() {
            if r.is_ascii_lowercase() || r.is_ascii_digit() {
                b.push(r);
            } else if !b.is_empty() && !b.ends_with('-') {
                b.push('-');
            }
        }
        b.trim_matches('-').to_owned()
    };
    let mut name = word(&e.label);
    if name.is_empty() {
        name = word(&e.template);
    }
    if name.is_empty() {
        "employee".clone_into(&mut name);
    }
    format!("{name}-{}", e.number)
}

/// The built-in subagent types card c's session keeps: none for a judge, the two that run as c's employee for every
/// other card.
pub fn builtin_agents_of(c: &Card) -> Vec<String> {
    if c.signs.verdict_role().is_none() {
        BUILTIN_SUBAGENTS.map(str::to_owned).to_vec()
    } else {
        Vec::new()
    }
}

/// An employee at work as a subagent in the session of a run: run is that run's token, handle the type it was offered
/// under, token the seat's own, and spawnee who a spawn of it works as.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Seat {
    run: String,
    handle: String,
    token: String,
    spawnee: Spawnee,
}

/// The seats of every live run: by seat token, to resolve a call, and by run token, to find a spawn's employee and to end
/// them with the run. It lives in `Shared`, under its lock.
#[derive(Debug, Default)]
pub struct SeatBook {
    by_token: HashMap<String, Seat>,
    /// run token -> handle -> seat
    by_run: HashMap<String, HashMap<String, Seat>>,
}

impl SeatBook {
    /// Makes the seats of run's session, handle -> who a spawn of it works as, and returns each one's token. A seat
    /// already held for the same employee under the same handle keeps its token, so a session opened again (a failover)
    /// leaves none dangling; a seat no longer offered ends.
    pub fn offer(&mut self, run: &str, offered: BTreeMap<String, Spawnee>) -> BTreeMap<String, String> {
        let held = self.by_run.remove(run).unwrap_or_default();
        let mut seats = HashMap::with_capacity(offered.len());
        let mut tokens = BTreeMap::new();
        for (handle, spawnee) in offered {
            let token = match held.get(&handle) {
                Some(s) if s.spawnee.employee == spawnee.employee => s.token.clone(),
                _ => new_id("t_"),
            };
            let seat = Seat {
                run: run.to_owned(),
                handle: handle.clone(),
                token: token.clone(),
                spawnee,
            };
            self.by_token.insert(token.clone(), seat.clone());
            tokens.insert(handle.clone(), token);
            seats.insert(handle, seat);
        }
        for (handle, s) in &held {
            if seats.get(handle).is_none_or(|now| now.token != s.token) {
                self.by_token.remove(&s.token);
            }
        }
        self.by_run.insert(run.to_owned(), seats);
        tokens
    }

    /// Ends every seat of run: its session has ended.
    pub fn revoke(&mut self, run: &str) {
        for s in self.by_run.remove(run).unwrap_or_default().into_values() {
            self.by_token.remove(&s.token);
        }
    }

    /// Who a spawn of type handle in run's session works as, when the session was offered it.
    pub fn offered(&self, run: &str, handle: &str) -> Option<Spawnee> {
        self.by_run.get(run)?.get(handle).map(|s| s.spawnee.clone())
    }
}

/// Who an MCP call comes from: a card's session, or a seat in it. `card` is the card whose session it is; `seat` is the
/// employee at work as a subagent of that card when the call comes from a seat, empty when it comes from the card itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Caller {
    pub card: Card,
    pub seat: String,
}

impl Core {
    /// Resolves an MCP bearer token to who it was issued to: a live run's card, or a seat of a live run.
    pub fn caller_for_token(&self, token: &str) -> Option<Caller> {
        let (run, seat) = {
            let sh = self.shared();
            let (run_token, seat) = match sh.seats.by_token.get(token) {
                Some(s) => (s.run.as_str(), s.spawnee.employee.clone()),
                None => (token, String::new()),
            };
            let run = sh.tokens.get(run_token).and_then(|id| sh.runs.get(id)).cloned();
            (run?, seat)
        };
        Some(Caller {
            card: run.card_now(),
            seat,
        })
    }

    /// The employee a spawn of type agent_type is at work as, in root's session, by spawner (root's card, or a child card
    /// in its process for a nested spawn): a built-in runs as the spawner's own employee, a handle as the employee root's
    /// session was offered under it. None: a type root's session was not given, refused. The spawner's own employee works
    /// with the spawner's loadout, another with what its type was built from.
    pub fn spawnee_of(&self, root: &Run, spawner: &Card, agent_type: &str) -> Option<Spawnee> {
        let own = Spawnee {
            employee: spawner.employee.clone(),
            agent: spawner.agent.clone(),
            loadout: spawner.loadout.clone(),
        };
        if BUILTIN_SUBAGENTS.contains(&agent_type) {
            return Some(own);
        }
        let seat = self.shared().seats.offered(&root.token, agent_type)?;
        Some(if seat.employee == spawner.employee { own } else { seat })
    }

    /// The card employee is at work in as a subagent in the process of card parent_id: the newest of that process's
    /// child cards that are employee's. What a seat records (its feedback) is that card's.
    pub fn seat_card(&self, parent_id: &str, employee: &str) -> Result<Card> {
        let root = self
            .run_of(parent_id)
            .ok_or_else(|| Error::refused(format!("the session you work in, {parent_id}'s, has ended")))?;
        let children: Vec<Arc<Run>> = root.state().children.values().cloned().collect();
        children
            .iter()
            .map(|c| c.card_now())
            .filter(|c| c.employee == employee)
            .max_by_key(|c| c.created_at)
            .ok_or_else(|| Error::refused("your card is not recorded yet: say it in your report"))
    }
}
