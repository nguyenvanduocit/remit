//! The chains the tabs show, each read and held once however many tabs show it (ADR-0095): its view and its events, as
//! the server answered them and as pushes keep them. A tab opening a chain held live is handed it at once; one that is
//! not is read, once for every tab that opens it meanwhile. What is pushed for a chain while it is read waits for the
//! answer and is applied over it, in the order it came. A chain stays held while a tab shows or opens it.

use std::collections::BTreeMap;

use link::{ErrorCode, Failure};
use model::{CardStatus, Origin};
use serde_json::Value;
use wire::app::{CHAIN_EVENTS_SHOWN, ChainView, EventPage, EventRead, LandingRead, Push, client};

use crate::calls::{Calls, Purpose};
use crate::catalog::Catalog;
use crate::events::{self, EventFacts};
use crate::index::{Index, newer};
use crate::models::{Models, Patch};
use crate::records::{Out, read, take, take_list};
use crate::shell::{Key, Model, Notice};

const ONE: &str = "";

#[derive(Debug, Clone, Copy)]
struct CardFacts {
    rev: i64,
    running: bool,
}

/// A read of a chain under way: its two answers, as each comes, and what was pushed meanwhile.
#[derive(Debug)]
struct Reading {
    seq: u64,
    view: Option<(ChainView, Value)>,
    events: Option<(EventPage, Value)>,
    /// Whether it was asked: once the server pushes the chain to this session (`homes_shown`), so nothing pushed
    /// between the read and the naming is missed.
    asked: bool,
    early: Vec<(Push, Value)>,
}

impl Reading {
    fn new(seq: u64) -> Self {
        Self {
            seq,
            view: None,
            events: None,
            asked: false,
            early: Vec::new(),
        }
    }
}

/// A read answered: the chain's events as it answered them, and what was pushed for the chain meanwhile, applied
/// over them, in the order it came. Each tab that opens the chain starts its windows on them.
pub(crate) struct Read {
    pub(crate) events: Vec<(EventRead, Value)>,
    pub(crate) early: Vec<(Push, Value)>,
}

/// What a tab that opens a chain gets.
pub(crate) enum Open {
    /// The chain is held live: shown at once.
    Held,
    /// The chain is being read; the tab waits for the answer.
    Read,
}

#[derive(Debug)]
struct HeldChain {
    /// A read answered, so its records are held.
    loaded: bool,
    /// Pushed to this session ever since it was read: a tab opening it is handed what is held. A chain no tab names
    /// in its homes any more, or a session gone, leaves it to be read again.
    live: bool,
    archived: bool,
    /// The chain's answer did not hold all of its events, or the oldest went as more were pushed.
    truncated: bool,
    read: Option<Reading>,
    chain: Index<i64>,
    cards: Index<CardFacts>,
    edges: Index<()>,
    decisions: Index<()>,
    drafts: Index<i64>,
    workflows: Index<i64>,
    helps: Index<i64>,
    agents: Index<i64>,
    verification: Index<i64>,
    away: Index<()>,
    retrospectives: Index<i64>,
    /// By landing id: the revisions of the landing and of its release (0: none), the newer copy kept.
    landings: Index<(i64, i64)>,
    datadog: Index<()>,
    events: Index<EventFacts>,
}

fn rows<T>(typed: Vec<T>, json: Vec<Value>, key: impl Fn(&T) -> Key) -> Vec<(Key, (), Value)> {
    typed.iter().zip(json).map(|(t, v)| (key(t), (), v)).collect()
}

fn revd<T>(
    typed: Vec<T>,
    json: Vec<Value>,
    key: impl Fn(&T) -> Key,
    rev: impl Fn(&T) -> i64,
) -> Vec<(Key, i64, Value)> {
    typed.iter().zip(json).map(|(t, v)| (key(t), rev(t), v)).collect()
}

/// The revisions a landing's copy is the newer by: its own, then its release's (a release moves without the landing).
fn landing_revs(l: &LandingRead) -> (i64, i64) {
    (l.landing.rev, l.release.as_ref().map_or(0, |r| r.rev))
}

fn upsert_newer(ix: &mut Index<i64>, id: Key, rev: i64, record: Value, out: &mut Vec<Patch>) {
    if newer(ix.get(&id).copied(), rev) {
        ix.put(id, rev, record, None, out);
    }
}

impl HeldChain {
    fn new(id: &str, seq: u64) -> Self {
        Self {
            loaded: false,
            live: false,
            archived: false,
            truncated: false,
            read: Some(Reading::new(seq)),
            chain: Index::of_chain(id, Model::OpenChain),
            cards: Index::of_chain(id, Model::Cards),
            edges: Index::of_chain(id, Model::Edges),
            decisions: Index::of_chain(id, Model::Decisions),
            drafts: Index::of_chain(id, Model::ChainDrafts),
            workflows: Index::of_chain(id, Model::Workflows),
            helps: Index::of_chain(id, Model::Helps),
            agents: Index::of_chain(id, Model::AgentStandings),
            verification: Index::of_chain(id, Model::Verification),
            away: Index::of_chain(id, Model::Away),
            retrospectives: Index::of_chain(id, Model::Retrospectives),
            landings: Index::of_chain(id, Model::Landings),
            datadog: Index::of_chain(id, Model::DatadogLinks),
            events: Index::of_chain(id, Model::ChainEvents),
        }
    }

    /// Every model it holds, let go.
    fn forget(&self, out: &mut Out) {
        let slots = [
            self.chain.slot(),
            self.cards.slot(),
            self.edges.slot(),
            self.decisions.slot(),
            self.drafts.slot(),
            self.workflows.slot(),
            self.helps.slot(),
            self.agents.slot(),
            self.verification.slot(),
            self.away.slot(),
            self.retrospectives.slot(),
            self.landings.slot(),
            self.datadog.slot(),
            self.events.slot(),
        ];
        for slot in slots {
            out.patches.push(Patch::Forget { slot: slot.clone() });
        }
    }

    /// Holds what a read answered; its events, as answered.
    fn show(
        &mut self,
        view: ChainView,
        mut json: Value,
        page: EventPage,
        mut page_json: Value,
        catalog: &mut Catalog,
        out: &mut Out,
    ) -> Vec<(EventRead, Value)> {
        // Where it stands lives in the catalog, as pushes keep it: the view's copy goes there (an archived chain has no
        // other).
        catalog.put_standing(&view.standing, take(&mut json, "standing"), out);
        let p = &mut out.patches;
        self.loaded = true;
        self.archived = view.chain.chain.archived_at.is_some();
        self.chain.reset(
            vec![(ONE.to_owned(), view.chain.chain.rev, take(&mut json, "chain"))],
            p,
        );
        let cards = view
            .cards
            .iter()
            .zip(take_list(&mut json, "cards"))
            .map(|(c, v)| {
                let facts = CardFacts {
                    rev: c.rev,
                    running: *c.status() == CardStatus::Running,
                };
                (c.id.clone(), facts, v)
            })
            .collect();
        self.cards.reset(cards, p);
        self.edges
            .reset(rows(view.edges, take_list(&mut json, "edges"), |e| e.id.clone()), p);
        self.decisions.reset(
            rows(view.decisions, take_list(&mut json, "decisions"), |d| {
                d.decision.id.clone()
            }),
            p,
        );
        self.drafts.reset(
            revd(view.drafts, take_list(&mut json, "drafts"), |d| d.id.clone(), |d| d.rev),
            p,
        );
        self.workflows.reset(
            revd(
                view.workflows,
                take_list(&mut json, "workflows"),
                |w| w.id.clone(),
                |w| w.rev,
            ),
            p,
        );
        self.helps.reset(
            revd(view.helps, take_list(&mut json, "helps"), |h| h.id.clone(), |h| h.rev),
            p,
        );
        self.agents.reset(
            revd(
                view.agents,
                take_list(&mut json, "agents"),
                |a| a.card_id.clone(),
                |a| a.rev,
            ),
            p,
        );
        self.verification.reset(
            vec![(ONE.to_owned(), view.verification.rev, take(&mut json, "verification"))],
            p,
        );
        let away = view.away.map(|_| (ONE.to_owned(), (), take(&mut json, "away")));
        self.away.reset(away.into_iter().collect(), p);
        self.retrospectives.reset(
            revd(
                view.retrospectives,
                take_list(&mut json, "retrospectives"),
                |r| r.retrospective.id.clone(),
                |r| r.retrospective.rev,
            ),
            p,
        );
        let landings = view
            .landings
            .iter()
            .zip(take_list(&mut json, "landings"))
            .map(|(l, v)| (l.landing.id.clone(), landing_revs(l), v))
            .collect();
        self.landings.reset(landings, p);
        self.datadog.reset(
            rows(view.datadog, take_list(&mut json, "datadog"), |l| l.label.clone()),
            p,
        );
        self.truncated = page.truncated;
        let events: Vec<(EventRead, Value)> = page
            .events
            .into_iter()
            .zip(take_list(&mut page_json, "events"))
            .collect();
        events::reset(&mut self.events, events.clone(), p);
        events
    }

    /// The server deleted the chain: nothing of it is held.
    fn close(&mut self, out: &mut Out) {
        if !self.loaded {
            return;
        }
        self.loaded = false;
        self.live = false;
        self.truncated = false;
        let p = &mut out.patches;
        self.chain.reset(Vec::new(), p);
        self.cards.reset(Vec::new(), p);
        self.edges.reset(Vec::new(), p);
        self.decisions.reset(Vec::new(), p);
        self.drafts.reset(Vec::new(), p);
        self.workflows.reset(Vec::new(), p);
        self.helps.reset(Vec::new(), p);
        self.agents.reset(Vec::new(), p);
        self.verification.reset(Vec::new(), p);
        self.away.reset(Vec::new(), p);
        self.retrospectives.reset(Vec::new(), p);
        self.landings.reset(Vec::new(), p);
        self.datadog.reset(Vec::new(), p);
        self.events.reset(Vec::new(), p);
    }

    fn apply(&mut self, push: &Push, record: Value, out: &mut Out) {
        let p = &mut out.patches;
        match push {
            Push::chain_updated { chain } => upsert_newer(&mut self.chain, ONE.to_owned(), chain.chain.rev, record, p),
            Push::card_updated { card } => {
                if newer(self.cards.get(&card.id).map(|c| c.rev), card.rev) {
                    let facts = CardFacts {
                        rev: card.rev,
                        running: *card.status() == CardStatus::Running,
                    };
                    self.cards.put(card.id.clone(), facts, record, None, p);
                }
            }
            // What a card shows of the moment, over the card it was pushed on: skipped when the card held is newer.
            Push::card_shown { shown } => {
                if self.cards.get(&shown.id).is_some_and(|c| c.rev <= shown.rev) {
                    p.push(Patch::Merge {
                        slot: self.cards.slot().clone(),
                        id: shown.id.clone(),
                        fields: shown_fields(shown),
                    });
                }
            }
            Push::agent_standing_updated { agent } => {
                upsert_newer(&mut self.agents, agent.card_id.clone(), agent.rev, record, p);
            }
            Push::edge_added { edge } => self.edges.put(edge.id.clone(), (), record, None, p),
            Push::decision_added { decision } => self.decisions.put(decision.decision.id.clone(), (), record, None, p),
            Push::verification_updated { verification } => {
                upsert_newer(&mut self.verification, ONE.to_owned(), verification.rev, record, p);
            }
            Push::workflow_updated { workflow } => {
                upsert_newer(&mut self.workflows, workflow.id.clone(), workflow.rev, record, p);
            }
            Push::retrospective_added { retrospective } => {
                upsert_newer(
                    &mut self.retrospectives,
                    retrospective.retrospective.id.clone(),
                    retrospective.retrospective.rev,
                    record,
                    p,
                );
            }
            Push::landing_updated { landing } => {
                let revs = landing_revs(landing);
                if self.landings.get(&landing.landing.id).is_none_or(|held| revs > *held) {
                    self.landings.put(landing.landing.id.clone(), revs, record, None, p);
                }
            }
            Push::help_updated { help } => upsert_newer(&mut self.helps, help.id.clone(), help.rev, record, p),
            Push::draft_updated { draft } => upsert_newer(&mut self.drafts, draft.id.clone(), draft.rev, record, p),
            Push::draft_deleted { gone } => {
                self.drafts.remove(&gone.id, p);
            }
            Push::event_added { event } | Push::event_updated { event } => {
                events::put(&mut self.events, event, record, p);
                if self.events.keep_last(CHAIN_EVENTS_SHOWN, p) {
                    self.truncated = true;
                }
            }
            Push::event_deleted { gone } => {
                self.events.remove(&events::key(gone.id), p);
            }
            Push::event_delta { delta } => events::grow(&mut self.events, delta, p),
            _ => {}
        }
    }
}

/// The chains held, by id.
#[derive(Debug, Default)]
pub(crate) struct HeldChains {
    held: BTreeMap<Key, HeldChain>,
    /// Counts reads: an answer to one no longer under way is dropped.
    seq: u64,
}

impl HeldChains {
    /// A tab opens chain `id`: handed at once when it is held live, else read (once, whoever else waits for it).
    pub(crate) fn open(&mut self, id: &str) -> Open {
        let seq = self.seq.wrapping_add(1);
        match self.held.get_mut(id) {
            Some(held) if held.read.is_some() => return Open::Read,
            Some(held) if held.live => return Open::Held,
            Some(held) => held.read = Some(Reading::new(seq)),
            None => {
                self.held.insert(id.to_owned(), HeldChain::new(id, seq));
            }
        }
        self.seq = seq;
        Open::Read
    }

    /// What streamed in the chain's home was let go as this session fell behind: it is read again, from now.
    pub(crate) fn read_again(&mut self, id: &str) {
        let seq = self.seq.wrapping_add(1);
        if let Some(held) = self.held.get_mut(id) {
            self.seq = seq;
            held.live = false;
            held.read = Some(Reading::new(seq));
        }
    }

    /// A new session's snapshot is in: what was pushed meanwhile is lost, so no chain is live until read again.
    pub(crate) fn stale(&mut self) {
        for held in self.held.values_mut() {
            held.live = false;
        }
    }

    /// Asks the reads waiting to be asked of the chains `readable` says the server pushes to this session from now on
    /// (its home named, and its `home_shown` come when named anew).
    pub(crate) fn read(&mut self, calls: &mut Calls, readable: impl Fn(&str) -> bool) {
        for (id, held) in &mut self.held {
            if !readable(id) {
                continue;
            }
            let Some(reading) = held.read.as_mut().filter(|r| !r.asked) else {
                continue;
            };
            reading.asked = true;
            let seq = reading.seq;
            calls.ask(
                client::chain_get(id.clone()),
                Purpose::ChainGet { chain: id.clone(), seq },
            );
            calls.ask(
                client::chain_events(id.clone()),
                Purpose::ChainEvents { chain: id.clone(), seq },
            );
        }
    }

    /// Keeps a chain while a tab shows or opens it, its read while a tab opens it, and it live while a tab names it in
    /// its homes.
    pub(crate) fn tend(&mut self, shown: &[&str], opening: &[&str], homes: &[&str], out: &mut Out) {
        self.held.retain(|id, held| {
            let opened = opening.contains(&id.as_str());
            if !opened && !shown.contains(&id.as_str()) {
                held.forget(out);
                return false;
            }
            if !opened {
                held.read = None;
            }
            if !homes.contains(&id.as_str()) {
                held.live = false;
            }
            true
        });
    }

    /// One of a read's two answers; how the read ended once it has. When the session went before it answered, the read
    /// goes on in the next.
    pub(crate) fn answered(
        &mut self,
        id: &str,
        seq: u64,
        result: Result<Value, Failure>,
        view: bool,
        catalog: &mut Catalog,
        out: &mut Out,
    ) -> Option<Result<Read, Failure>> {
        let held = self.held.get_mut(id)?;
        let reading = held.read.as_mut().filter(|r| r.seq == seq)?;
        let read_one = result.and_then(|json| {
            if view {
                reading.view = Some((read(&json)?, json));
            } else {
                reading.events = Some((read(&json)?, json));
            }
            Ok(())
        });
        // The session went before it answered: the chain is named and read in the next one. Let go, the next session
        // would be pushed it before naming it, then named without it, and a push sent before could land on its read.
        if let Err(failure) = read_one {
            if failure.code == ErrorCode::Disconnected {
                *reading = Reading::new(seq);
                return None;
            }
            held.read = None;
            return Some(Err(failure));
        }
        if reading.view.is_none() || reading.events.is_none() {
            return None;
        }
        let reading = held.read.take()?;
        let ((view, view_json), (page, page_json)) = (reading.view?, reading.events?);
        let events = held.show(view, view_json, page, page_json, catalog, out);
        held.live = true;
        for (push, record) in &reading.early {
            held.apply(push, record.clone(), out);
        }
        Some(Ok(Read {
            events,
            early: reading.early,
        }))
    }

    /// The server deleted the chain: nothing of it is held.
    pub(crate) fn close(&mut self, id: &str, out: &mut Out) {
        if let Some(held) = self.held.get_mut(id) {
            held.close(out);
        }
    }

    /// Whether the chain is held archived (the snapshot lists none).
    pub(crate) fn archived(&self, id: &str) -> bool {
        self.held.get(id).is_some_and(|h| h.loaded && h.archived)
    }

    /// Whether what is held of the chain's events lacks some of its past.
    pub(crate) fn truncated(&self, id: Option<&str>) -> bool {
        id.and_then(|id| self.held.get(id)).is_some_and(|h| h.truncated)
    }

    /// The chain's events held, oldest first, as the models hold them: what a tab handed the chain at once starts its
    /// windows on. What this step changed is not in the models until the step ends, so a tab's ask reads them, which
    /// comes first in its step.
    pub(crate) fn events(&self, id: &str, models: &Models) -> Vec<(EventRead, Value)> {
        let Some(held) = self.held.get(id) else {
            return Vec::new();
        };
        models
            .rows(held.events.slot())
            .filter_map(|(_, v)| read::<EventRead>(v).ok().map(|e| (e, v.clone())))
            .collect()
    }

    /// An agent finishing hands its report to the Chair: worth the owner's notice. Read before the push is applied.
    pub(crate) fn turn_finished(&self, push: &Push) -> Option<Notice> {
        let Push::card_updated { card } = push else { return None };
        let prev = self.held.get(push.home()?)?.cards.get(&card.id)?;
        (prev.rev < card.rev && prev.running && *card.status() == CardStatus::Idle && card.origin != Origin::Chair)
            .then(|| Notice::TurnFinished {
                name: card.name.clone(),
            })
    }

    /// A push about a chain: applied to it when held, kept while it is read, ignored otherwise.
    pub(crate) fn on_push(&mut self, push: &Push, json: &Value, out: &mut Out) {
        let Some(held) = push.home().and_then(|home| self.held.get_mut(home)) else {
            return;
        };
        if let Some(reading) = held.read.as_mut() {
            reading.early.push((push.clone(), json.clone()));
        } else if held.loaded {
            held.apply(push, json.clone(), out);
        }
    }

    /// A draft the owner's own save or deletion answered, applied as its push would be.
    pub(crate) fn put_draft(&mut self, home: &str, id: Key, rev: i64, record: Value, out: &mut Out) {
        if let Some(held) = self.held.get_mut(home).filter(|h| h.loaded) {
            upsert_newer(&mut held.drafts, id, rev, record, &mut out.patches);
        }
    }

    /// The revision of the server's copy of draft id held here.
    pub(crate) fn draft_rev(&self, id: &str) -> Option<i64> {
        self.held.values().find_map(|h| h.drafts.get(id).copied())
    }

    pub(crate) fn drop_draft(&mut self, id: &str, out: &mut Out) {
        for held in self.held.values_mut() {
            held.drafts.remove(id, &mut out.patches);
        }
    }
}

/// The fields a `card_shown` sets on its card.
pub(crate) fn shown_fields(shown: &wire::app::CardShown) -> Value {
    serde_json::json!({ "activity": shown.activity, "ctxUsed": shown.ctx_used, "ctxSize": shown.ctx_size })
}
