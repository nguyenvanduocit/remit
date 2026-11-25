//! The chain a tab opens (`openChain.ts`): which it shows, which it is opening, and its own window on each card's events.
//! The chain's view and its events are held once for every tab that shows it (`held.rs`); a tab keeps only how far back
//! it read each card. The route opens it, a reconnect opens it again, its deletion closes it. Opening chains one after
//! another ends on the last asked for: an earlier read answering late is not shown.

use std::collections::{BTreeSet, HashMap, HashSet};

use link::{ErrorCode, Failure};
use serde_json::Value;
use wire::app::{CARD_EVENTS_PAGE, EventPage, EventRead, Push, client};

use crate::calls::{Calls, Purpose};
use crate::catalog::Catalog;
use crate::events::{self, EventFacts};
use crate::index::Index;
use crate::io::Tab;
use crate::models::{Owner, Patch, Slot};
use crate::records::{Out, read, take_list};
use crate::shell::{Key, Model, Notice};

/// How an opening ended, for whoever asked for it.
#[derive(Debug)]
pub(crate) struct Opened {
    pub(crate) by_sync: Option<u64>,
    pub(crate) result: Result<(), Failure>,
}

#[derive(Debug)]
pub(crate) struct OpenChain {
    /// The tab that opened it: its windows are kept for that tab alone.
    tab: Tab,
    shown: Option<Key>,
    /// The chain being opened, waiting for its read; empty when none is.
    pub(crate) opening: Key,
    /// The chain last asked to open: what a reconnect opens again, even if its opening was cut short.
    wanted: Key,
    /// The sync attempt the opening under way is part of, when a reconnect asked for it.
    by_sync: Option<u64>,
    by_card: HashMap<Key, Index<EventFacts>>,
    /// The cards whose whole stream was asked for.
    whole: HashSet<Key>,
    pub(crate) earlier: BTreeSet<Key>,
    /// The cards whose earlier page is being read.
    reading: HashSet<Key>,
    /// The pages of its past the owner read back, by card: a card keeps that many pages over its newest as it is pushed
    /// more (`CARD_EVENTS_PAGE`), however long its agent goes on.
    read_back: HashMap<Key, usize>,
    /// Counts the windows started anew: a page asked for before is dropped.
    generation: u64,
}

impl OpenChain {
    /// The chain tab `tab` opens; none yet.
    pub(crate) fn new(tab: Tab) -> Self {
        Self {
            tab,
            shown: None,
            opening: Key::new(),
            wanted: Key::new(),
            by_sync: None,
            by_card: HashMap::new(),
            whole: HashSet::new(),
            earlier: BTreeSet::new(),
            reading: HashSet::new(),
            read_back: HashMap::new(),
            generation: 0,
        }
    }

    /// The chain shown, if one is.
    pub(crate) fn shown(&self) -> Option<&str> {
        self.shown.as_deref()
    }

    /// Opens chain id: the core hands it what is held of it, or reads it. An opening that replaces one a reconnect's
    /// sync waits for (the owner opened another chain, its home was reset) is the one that sync waits for.
    pub(crate) fn open(&mut self, id: Key, by_sync: Option<u64>) {
        self.by_sync = by_sync.or_else(|| self.by_sync());
        self.wanted.clone_from(&id);
        self.opening = id;
    }

    /// The sync attempt an opening under way is part of, when a reconnect asked for it.
    pub(crate) fn by_sync(&self) -> Option<u64> {
        self.by_sync.filter(|_| !self.opening.is_empty())
    }

    /// The chain this tab shows or is opening: the home whose pushes the session is to be pushed.
    pub(crate) fn home(&self) -> Option<&str> {
        if self.opening.is_empty() {
            self.shown()
        } else {
            Some(&self.opening)
        }
    }

    /// After a reconnect, once the snapshot is in: what was pushed meanwhile is lost, so the chain last asked for is
    /// opened again, or else the one shown. One the snapshot no longer lists was deleted meanwhile and is closed, unless
    /// it is archived (the snapshot lists none): the server is asked, and one it no longer has (`NOT_FOUND`) is closed
    /// then. The chain to open again, if any.
    pub(crate) fn reload(&mut self, catalog: &Catalog, archived: impl Fn(&str) -> bool, out: &mut Out) -> Option<Key> {
        if let Some(shown) = self.shown.clone()
            && !catalog.listed(&shown)
            && !archived(&shown)
        {
            self.close(&shown, out);
        }
        if catalog.listed(&self.wanted) {
            Some(self.wanted.clone())
        } else {
            self.shown.clone()
        }
    }

    /// The chain being opened is held: shown, its windows started on the chain's events held. The sync it ends, if any.
    pub(crate) fn opened(&mut self, events: Vec<(EventRead, Value)>, out: &mut Out) -> Option<u64> {
        self.shown = Some(std::mem::take(&mut self.opening));
        self.hold(events, &mut out.patches);
        self.by_sync.take()
    }

    /// Reading the chain being opened failed. During a reconnect's sync, a chain the server no longer has
    /// (`NOT_FOUND`) was deleted meanwhile: it is closed, and the sync goes on.
    pub(crate) fn failed(&mut self, failure: Failure, out: &mut Out) -> Opened {
        let id = std::mem::take(&mut self.opening);
        let by_sync = self.by_sync.take();
        if by_sync.is_some() && failure.code == ErrorCode::NotFound {
            self.close(&id, out);
            return Opened {
                by_sync,
                result: Ok(()),
            };
        }
        Opened {
            by_sync,
            result: Err(failure),
        }
    }

    /// The server deleted the chain: if this tab shows it, nothing of it stays on screen.
    pub(crate) fn close(&mut self, id: &str, out: &mut Out) {
        if self.shown() != Some(id) {
            return;
        }
        self.shown = None;
        self.hold(Vec::new(), &mut out.patches);
    }

    /// Its windows, started anew on the chain's events, filed by card.
    fn hold(&mut self, events: Vec<(EventRead, Value)>, p: &mut Vec<Patch>) {
        self.generation = self.generation.wrapping_add(1);
        for (card, _) in self.by_card.drain() {
            p.push(Patch::Forget {
                slot: Slot {
                    owner: Owner::Tab(self.tab),
                    model: Model::CardEvents(card),
                },
            });
        }
        let mut filed: HashMap<Key, Vec<(EventRead, Value)>> = HashMap::new();
        for (e, v) in events {
            filed.entry(e.event.card_id.clone()).or_default().push((e, v));
        }
        let mut cards: Vec<_> = filed.into_iter().collect();
        cards.sort_by(|a, b| a.0.cmp(&b.0));
        for (card, mine) in cards {
            let mut ix = Index::of(self.tab, Model::CardEvents(card.clone()));
            events::reset(&mut ix, mine, p);
            self.by_card.insert(card, ix);
        }
        self.whole.clear();
        self.earlier.clear();
        self.read_back.clear();
    }

    /// A card's stream, for its drawer, when the chain's answer did not hold all of it.
    pub(crate) fn load_stream(&mut self, calls: &mut Calls, card: Key, truncated: bool) {
        if !truncated || self.whole.contains(&card) {
            return;
        }
        self.whole.insert(card.clone());
        let purpose = Purpose::CardStream {
            tab: self.tab,
            card: card.clone(),
            generation: self.generation,
        };
        calls.ask(client::card_events(card, 0), purpose);
    }

    pub(crate) fn stream_answered(
        &mut self,
        card: Key,
        generation: u64,
        result: Result<Value, Failure>,
        out: &mut Out,
    ) {
        let page = result.and_then(|mut json| {
            let page: EventPage = read(&json)?;
            Ok((page, take_list(&mut json, "events")))
        });
        match page {
            Ok((page, json)) => {
                if generation != self.generation {
                    return;
                }
                let mut ix = Index::of(self.tab, Model::CardEvents(card.clone()));
                events::reset(&mut ix, page.events.into_iter().zip(json).collect(), &mut out.patches);
                self.by_card.insert(card.clone(), ix);
                if page.truncated {
                    self.earlier.insert(card);
                }
            }
            Err(failure) => {
                self.whole.remove(&card);
                out.notice_to(
                    self.tab,
                    Notice::CouldNot {
                        what: crate::shell::What::LoadStream,
                        failure,
                    },
                );
            }
        }
    }

    /// The card's page of events before the oldest held, as the owner asks for more of its past.
    pub(crate) fn load_earlier(&mut self, calls: &mut Calls, card: Key) {
        let Some(oldest) = self
            .by_card
            .get(&card)
            .and_then(|ix| ix.keys().next().map(|(_, f)| f.id))
        else {
            return;
        };
        if !self.earlier.contains(&card) || self.reading.contains(&card) {
            return;
        }
        self.reading.insert(card.clone());
        let purpose = Purpose::CardEarlier {
            tab: self.tab,
            card: card.clone(),
            generation: self.generation,
        };
        calls.ask(client::card_events(card, oldest), purpose);
    }

    pub(crate) fn earlier_answered(
        &mut self,
        card: Key,
        generation: u64,
        result: Result<Value, Failure>,
        out: &mut Out,
    ) {
        self.reading.remove(&card);
        let page = result.and_then(|mut json| {
            let page: EventPage = read(&json)?;
            Ok((page, take_list(&mut json, "events")))
        });
        match page {
            Ok((page, json)) => {
                if generation != self.generation {
                    return;
                }
                let read = self.read_back.entry(card.clone()).or_default();
                *read = read.saturating_add(1);
                if let Some(ix) = self.by_card.get_mut(&card) {
                    let oldest = ix.first().cloned();
                    for (e, v) in page.events.iter().zip(json) {
                        ix.put(
                            events::key(e.event.id),
                            EventFacts::of(e),
                            v,
                            oldest.clone(),
                            &mut out.patches,
                        );
                    }
                }
                if !page.truncated {
                    self.earlier.remove(&card);
                }
            }
            Err(failure) => out.notice_to(
                self.tab,
                Notice::CouldNot {
                    what: crate::shell::What::LoadEarlier,
                    failure,
                },
            ),
        }
    }

    /// A push about the chain this tab shows: its windows on its cards' events take it. While the tab opens the chain
    /// they wait: they start anew on the chain's events once it is held.
    pub(crate) fn on_push(&mut self, push: &Push, json: &Value, out: &mut Out) {
        let Some(chain) = push.home() else { return };
        if self.opening == chain || self.shown() != Some(chain) {
            return;
        }
        let p = &mut out.patches;
        match push {
            Push::event_added { event } | Push::event_updated { event } => {
                let tab = self.tab;
                let card = &event.event.card_id;
                let mine = self
                    .by_card
                    .entry(card.clone())
                    .or_insert_with(|| Index::of(tab, Model::CardEvents(card.clone())));
                events::put(mine, event, json.clone(), p);
                let pages = self.read_back.get(card).copied().unwrap_or_default().saturating_add(1);
                if mine.keep_last(CARD_EVENTS_PAGE.saturating_mul(pages), p) {
                    self.earlier.insert(card.clone());
                }
            }
            Push::event_deleted { gone } => {
                if let Some(mine) = self.by_card.get_mut(&gone.card_id) {
                    mine.remove(&events::key(gone.id), p);
                }
            }
            Push::event_delta { delta } => {
                if let Some(mine) = self.by_card.get_mut(&delta.card_id) {
                    events::grow(mine, delta, p);
                }
            }
            _ => {}
        }
    }
}
