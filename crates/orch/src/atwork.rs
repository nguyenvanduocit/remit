//! "At work now": every seat across the chains (and the Assistant) that runs or waits, laid out by the server from
//! where each agent stands (agentstanding.rs), so the owner sees in one place who is on what, what it uses this minute,
//! since when, and whether its time counts as work or as a wait with a known reason, and stops one with a why
//! (`Orchestrator::stop_card`). Kept beside the views it is computed with (views.rs's `Computed`) and pushed whole when it
//! changes. Pure: `seats_of`, `Kept`. At the edge: `push_at_work`, `at_work`.

use std::collections::HashMap;

use model::{Card, CardStatus, Chain, ChainStatus, Origin, Time};
use wire::app::{AgentStanding, Ask, AtWork, Seat, SeatKind};

use crate::agentstanding::at_work_now;
use crate::orch::Orchestrator;
use chains::{Error, Result};
use wire::app::AgentTone;

/// A seat as laid out, with what orders it among the rest: since when it stands as its line says (`at`, else when its
/// card was born), then when its card was born.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Placed {
    seat: Seat,
    at: Time,
    born: Time,
}

/// The seats of chain c (none for the Assistant) whose cards are cards and whose standings are agents: each card that
/// runs or opens a session, asks a permission, waits for a provider or on the owner, or awaits the Chair's acceptance,
/// and the Chair of a chain that waits. A finished chain has none.
pub(crate) fn seats_of(c: Option<&Chain>, cards: &[Card], agents: &[AgentStanding]) -> Vec<Placed> {
    if c.is_some_and(|c| *c.status() == ChainStatus::Done) {
        return Vec::new();
    }
    let by_id: HashMap<&str, &AgentStanding> = agents.iter().map(|a| (a.card_id.as_str(), a)).collect();
    let in_view = |k: &Card, a: &AgentStanding| {
        at_work_now(k)
            || k.status() == &CardStatus::Waiting
            || k.awaiting_review
            || !a.waiting.is_empty()
            || matches!(a.tone, AgentTone::Running | AgentTone::Waiting)
            || (k.origin == Origin::Chair && c.is_some_and(|c| *c.status() == ChainStatus::Waiting))
    };
    let shown: Vec<(&Card, &AgentStanding)> = cards
        .iter()
        .filter_map(|k| by_id.get(k.id.as_str()).map(|a| (k, *a)))
        .filter(|(k, a)| in_view(k, a))
        .collect();
    shown
        .iter()
        .map(|(k, a)| Placed {
            seat: Seat {
                card_id: k.id.clone(),
                home: a.home.clone(),
                kind: match k.origin {
                    Origin::Assistant => SeatKind::Assistant,
                    Origin::Chair => SeatKind::Chair,
                    _ => SeatKind::Agent,
                },
                chain_title: c.map(|c| c.title.clone()).unwrap_or_default(),
                name: k.name.clone(),
                employee: k.employee.clone(),
                label: a.label.clone(),
                tone: a.tone,
                work: a.work.clone(),
                doing: a.doing.clone(),
                doing_tone: a.doing_tone,
                since: a.since.map(|t| t.rfc3339()),
                warms: a.warms,
                helpers: a
                    .children
                    .iter()
                    .filter(|id| shown.iter().any(|(s, _)| s.id == **id))
                    .cloned()
                    .collect(),
                stop: a.asks.contains(&Ask::Stop),
                stop_says: a.stop_says.clone(),
            },
            at: a.since.unwrap_or(k.created_at),
            born: k.created_at,
        })
        .collect()
}

/// The seats last laid out, by home, and the view last pushed.
#[derive(Debug, Default)]
pub(crate) struct Kept {
    by_home: HashMap<String, Vec<Placed>>,
    shown: AtWork,
}

impl Kept {
    /// Keeps seats as home's; the view laid out again, at a new revision from rev, when it changed (to push).
    pub(crate) fn put(&mut self, home: &str, seats: Vec<Placed>, rev: &mut i64) -> Option<AtWork> {
        if self.by_home.get(home).is_some_and(|was| *was == seats)
            || (seats.is_empty() && !self.by_home.contains_key(home))
        {
            self.by_home.entry(home.to_owned()).or_default();
            return None;
        }
        self.by_home.insert(home.to_owned(), seats);
        self.lay_out(rev)
    }

    /// Drops the seats of the homes held says are gone; the view laid out again when it changed (to push).
    pub(crate) fn forget(&mut self, held: impl Fn(&str) -> bool, rev: &mut i64) -> Option<AtWork> {
        let before = self.by_home.len();
        self.by_home.retain(|home, _| held(home));
        if self.by_home.len() == before {
            return None;
        }
        self.lay_out(rev)
    }

    /// Whether home's seats have been laid out since this process started.
    pub(crate) fn knows(&self, home: &str) -> bool {
        self.by_home.contains_key(home)
    }

    pub(crate) fn shown(&self) -> AtWork {
        self.shown.clone()
    }

    fn lay_out(&mut self, rev: &mut i64) -> Option<AtWork> {
        let mut placed: Vec<&Placed> = self.by_home.values().flatten().collect();
        placed.sort_by(|a, b| {
            rank(&a.seat)
                .cmp(&rank(&b.seat))
                .then_with(|| a.at.cmp(&b.at))
                .then_with(|| a.seat.home.cmp(&b.seat.home))
                .then_with(|| a.born.cmp(&b.born))
        });
        let seats: Vec<Seat> = placed.into_iter().map(|p| p.seat.clone()).collect();
        if seats == self.shown.seats {
            return None;
        }
        *rev = rev.saturating_add(1);
        self.shown = AtWork { seats, rev: *rev };
        Some(self.shown.clone())
    }
}

/// Where a seat goes in the view: what waits on the owner first, then what runs, then the rest.
fn rank(s: &Seat) -> u8 {
    match s.tone {
        AgentTone::Needs | AgentTone::Failed => 0,
        _ if s.warms || s.tone == AgentTone::Running => 1,
        _ => 2,
    }
}

impl Orchestrator {
    /// Pushes v, the at-work view laid out again, when there is one.
    pub(crate) fn push_at_work(&self, v: Option<AtWork>) {
        if let Some(v) = v {
            self.notify.at_work_updated(&v);
        }
    }

    /// The at-work view as it is now: the chains that run, wait or are paused whose seats were not laid out since this
    /// process started are laid out first.
    pub async fn at_work(&self) -> Result<AtWork> {
        let chains = self.store.read(|r| r.chains()).await?;
        for c in chains.iter().filter(|c| {
            c.archived_at.is_none()
                && matches!(
                    c.status(),
                    ChainStatus::Running | ChainStatus::Waiting | ChainStatus::Paused
                )
        }) {
            if self.views.compute.lock().await.at_work.knows(&c.id) {
                continue;
            }
            match self.chain_of(&c.id).await {
                Ok(_) | Err(Error::Store(store::Error::NotFound) | Error::ChainDeleted) => {}
                Err(err) => return Err(err),
            }
        }
        self.assistant_of().await?;
        Ok(self.views.compute.lock().await.at_work.shown())
    }
}
