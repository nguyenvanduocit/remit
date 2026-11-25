//! The replica of the Assistant (`assistant.ts`): its card, where it stands and the owner's unsent words to it (the
//! snapshot's `AssistantView`), its stream, which is the conversation, and its feed, sent whole each time it moves. The
//! view lives as long as the connection; the stream is asked for when the owner first opens the Assistant, and again
//! after a reconnect once it has been.

use link::Failure;
use serde_json::Value;
use wire::app::{Activity, AssistantView, EventPage, Push, client};

use crate::calls::{Calls, Purpose};
use crate::events::{self, EventFacts};
use crate::held::shown_fields;
use crate::index::{Index, newer};
use crate::models::Patch;
use crate::records::{Out, read, take, take_list};
use crate::shell::{Key, Model};

const ONE: &str = "";

#[derive(Debug)]
pub(crate) struct Assistant {
    /// The Assistant's id (the home of its records) and its card's.
    home: Option<(Key, Key)>,
    pub(crate) loaded: bool,
    pub(crate) loading: bool,
    seq: u64,
    wanted: bool,
    /// The stream is to be read once the server pushes the Assistant's home to this session (`homes_shown`).
    unread: bool,
    early: Vec<(Push, Value)>,
    view: Index<()>,
    card: Index<i64>,
    agent: Index<i64>,
    drafts: Index<i64>,
    events: Index<EventFacts>,
    feed: Index<()>,
}

impl Default for Assistant {
    fn default() -> Self {
        Self {
            home: None,
            loaded: false,
            loading: false,
            seq: 0,
            wanted: false,
            unread: false,
            early: Vec::new(),
            view: Index::new(Model::Assistant),
            card: Index::new(Model::AssistantCard),
            agent: Index::new(Model::AssistantAgent),
            drafts: Index::new(Model::AssistantDrafts),
            events: Index::new(Model::AssistantEvents),
            feed: Index::new(Model::AssistantFeed),
        }
    }
}

impl Assistant {
    /// The Assistant's id: the home of what is its own.
    pub(crate) fn home(&self) -> Option<&str> {
        self.home.as_ref().map(|(home, _)| home.as_str())
    }

    /// The Assistant as the snapshot has it: its card, standing and drafts replace what was held.
    pub(crate) fn set(&mut self, view: &AssistantView, mut json: Value, out: &mut Out) {
        let p = &mut out.patches;
        self.home = Some((view.assistant.id.clone(), view.card.id.clone()));
        self.view
            .reset(vec![(ONE.to_owned(), (), take(&mut json, "assistant"))], p);
        self.card
            .reset(vec![(ONE.to_owned(), view.card.rev, take(&mut json, "card"))], p);
        self.agent
            .reset(vec![(ONE.to_owned(), view.agent.rev, take(&mut json, "agent"))], p);
        let drafts = view
            .drafts
            .iter()
            .zip(take_list(&mut json, "drafts"))
            .map(|(d, v)| (d.id.clone(), d.rev, v))
            .collect();
        self.drafts.reset(drafts, p);
    }

    /// No Assistant in the snapshot (a mentor's session): nothing of one is held.
    pub(crate) fn forget(&mut self, out: &mut Out) {
        let p = &mut out.patches;
        self.home = None;
        self.view.reset(Vec::new(), p);
        self.card.reset(Vec::new(), p);
        self.agent.reset(Vec::new(), p);
        self.drafts.reset(Vec::new(), p);
    }

    /// The feed as the server last sent it, whole.
    pub(crate) fn set_feed(&mut self, json: Value, out: &mut Out) {
        self.feed.reset(vec![(ONE.to_owned(), (), json)], &mut out.patches);
    }

    /// Wants the stream: it is read (`read`) once the server pushes the Assistant's home to this session, and what is
    /// pushed for it meanwhile waits for the answer and is applied over it. Whether it is wanted (not before the first
    /// snapshot names the Assistant's card).
    pub(crate) fn load(&mut self) -> bool {
        if self.home.is_none() {
            return false;
        }
        self.seq = self.seq.wrapping_add(1);
        self.wanted = true;
        self.loading = true;
        self.unread = true;
        self.early.clear();
        true
    }

    /// Reads the stream asked for, once the server pushes the Assistant's home to this session.
    pub(crate) fn read(&mut self, calls: &mut Calls) {
        let Some((_, card)) = self.home.clone().filter(|_| self.unread) else {
            return;
        };
        self.unread = false;
        calls.ask(client::card_events(card, 0), Purpose::AssistantStream { seq: self.seq });
    }

    /// After a reconnect, once the snapshot is in: a stream held is asked for again. Whether it was.
    pub(crate) fn reload(&mut self) -> bool {
        self.wanted && self.load()
    }

    /// The stream's answer; none for one a newer ask replaced.
    pub(crate) fn answered(
        &mut self,
        seq: u64,
        result: Result<Value, Failure>,
        out: &mut Out,
    ) -> Option<Result<(), Failure>> {
        if seq != self.seq {
            return None;
        }
        self.loading = false;
        let early = std::mem::take(&mut self.early);
        let page = result.and_then(|mut json| {
            let page: EventPage = read(&json)?;
            Ok((page, take_list(&mut json, "events")))
        });
        match page {
            Ok((page, json)) => {
                events::reset(
                    &mut self.events,
                    page.events.into_iter().zip(json).collect(),
                    &mut out.patches,
                );
                self.loaded = true;
                for (push, record) in early {
                    self.on_push(&push, &record, out);
                }
                Some(Ok(()))
            }
            Err(failure) => Some(Err(failure)),
        }
    }

    fn ours(&self, home: &str) -> bool {
        self.home() == Some(home)
    }

    fn card_is(&self, home: &str, card: &str) -> bool {
        self.home.as_ref().is_some_and(|(h, c)| h == home && c == card)
    }

    /// A push for the Assistant's home; pushes for any other home are not its own.
    pub(crate) fn on_push(&mut self, push: &Push, json: &Value, out: &mut Out) {
        if !push.home().is_some_and(|home| self.ours(home)) {
            return;
        }
        let record = json.clone();
        let p = &mut out.patches;
        match push {
            Push::card_updated { card } if self.card_is(&card.home, &card.id) => {
                if newer(self.card.get(ONE).copied(), card.rev) {
                    self.card.put(ONE.to_owned(), card.rev, record, None, p);
                }
            }
            Push::card_shown { shown } if self.card_is(&shown.home, &shown.id) => {
                if self.card.get(ONE).is_some_and(|rev| *rev <= shown.rev) {
                    p.push(Patch::Merge {
                        slot: self.card.slot().clone(),
                        id: ONE.to_owned(),
                        fields: shown_fields(shown),
                    });
                }
            }
            Push::agent_standing_updated { agent } if self.card_is(&agent.home, &agent.card_id) => {
                if newer(self.agent.get(ONE).copied(), agent.rev) {
                    self.agent.put(ONE.to_owned(), agent.rev, record, None, p);
                }
            }
            Push::draft_updated { draft } if self.ours(&draft.home) => {
                if newer(self.drafts.get(&draft.id).copied(), draft.rev) {
                    self.drafts.put(draft.id.clone(), draft.rev, record, None, p);
                }
            }
            Push::draft_deleted { gone } if self.ours(&gone.home) => {
                self.drafts.remove(&gone.id, p);
            }
            Push::event_added { event } | Push::event_updated { event } if self.ours(&event.event.home) => {
                self.on_stream(push, record, out, |me, out, record| {
                    // A Start over begins the conversation anew: the server's page of it begins at the newest one.
                    if event.activity == Some(Activity::StartedOver) {
                        events::reset(&mut me.events, vec![(event.clone(), record)], &mut out.patches);
                    } else {
                        events::put(&mut me.events, event, record, &mut out.patches);
                    }
                });
            }
            Push::event_deleted { gone } if self.ours(&gone.home) => {
                self.on_stream(push, record, out, |me, out, _| {
                    me.events.remove(&events::key(gone.id), &mut out.patches);
                });
            }
            Push::event_delta { delta } if self.ours(&delta.home) => {
                self.on_stream(push, record, out, |me, out, _| {
                    events::grow(&mut me.events, delta, &mut out.patches);
                });
            }
            _ => {}
        }
    }

    /// A push about the stream: applied once it is held, waiting for it while it is asked for, dropped before (asking
    /// brings it).
    fn on_stream(&mut self, push: &Push, record: Value, out: &mut Out, apply: impl FnOnce(&mut Self, &mut Out, Value)) {
        if self.loading {
            self.early.push((push.clone(), record));
        } else if self.loaded {
            apply(self, out, record);
        }
    }

    /// A draft the owner's own save answered, applied as its push would be.
    pub(crate) fn put_draft(&mut self, home: &str, id: Key, rev: i64, record: Value, out: &mut Out) {
        if self.ours(home) && newer(self.drafts.get(&id).copied(), rev) {
            self.drafts.put(id, rev, record, None, &mut out.patches);
        }
    }

    /// The revision of the server's copy of draft id held here.
    pub(crate) fn draft_rev(&self, id: &str) -> Option<i64> {
        self.drafts.get(id).copied()
    }

    pub(crate) fn drop_draft(&mut self, id: &str, out: &mut Out) {
        self.drafts.remove(id, &mut out.patches);
    }
}
