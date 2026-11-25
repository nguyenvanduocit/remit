//! The records the core keeps, by model and in each model's order (ADR-0095 decision 5), and what a tab that shows a
//! model is told of it: the model whole when it is shown or read again, then only the rows that changed, so a streamed
//! chunk crosses as the chunk and the rows that did not change are never copied again. A model the browser's tabs share
//! (the catalog's, the Assistant's, the owner's typed words and sends) is kept once and told to every tab that shows it;
//! a chain's view and events are kept once and told to every tab that shows that chain; a tab's own (its window on each
//! card's events, the event and the archived pages it opened) is kept for that tab and told to it alone (decision 4). The rules (`Index` and its users) say every change as a `Patch`; this applies it, and decides nothing.

use std::collections::{BTreeMap, HashMap};

use indexmap::IndexMap;
use serde_json::Value;

use crate::io::Tab;
use crate::shell::{Change, Freshness, Key, Model, Row, Told};

/// Who a model's records are kept for: every tab, a tab alone (what it opened), or the tabs that show one chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sharing {
    Every,
    Tab,
    Chain,
}

fn sharing(model: &Model) -> Sharing {
    match model {
        Model::OpenChain
        | Model::Cards
        | Model::Edges
        | Model::Decisions
        | Model::ChainDrafts
        | Model::Workflows
        | Model::Helps
        | Model::AgentStandings
        | Model::Verification
        | Model::Away
        | Model::Retrospectives
        | Model::Landings
        | Model::DatadogLinks
        | Model::ChainEvents => Sharing::Chain,
        Model::CardEvents(_) | Model::OpenEvent | Model::Archived | Model::Repo | Model::Repos => Sharing::Tab,
        Model::Who
        | Model::Chains
        | Model::Inbox
        | Model::Standings
        | Model::Board
        | Model::Playbooks
        | Model::Agents
        | Model::Skills
        | Model::Sources
        | Model::Capabilities
        | Model::Health
        | Model::Workload
        | Model::AtWork
        | Model::Signals
        | Model::Boards
        | Model::Insights
        | Model::Events
        | Model::EventSignals
        | Model::Sensors
        | Model::ClientKeys
        | Model::Extensions
        | Model::Feedback
        | Model::Objectives
        | Model::Projects
        | Model::Employees
        | Model::Mentors
        | Model::MentorStandings
        | Model::Assistant
        | Model::AssistantCard
        | Model::AssistantAgent
        | Model::AssistantDrafts
        | Model::AssistantEvents
        | Model::AssistantFeed
        | Model::Typed
        | Model::Outbox => Sharing::Every,
    }
}

/// Whose records a slot holds.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Owner {
    Every,
    Tab(Tab),
    /// The chain's view and events, held once for every tab that shows it.
    Chain(Key),
}

/// Where a model's records are kept.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Slot {
    pub(crate) owner: Owner,
    pub(crate) model: Model,
}

/// One change to a model's records, as a rule says it; a batch of them is applied in order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Patch {
    /// The model holds these records, in this order, and nothing else.
    Reset {
        slot: Slot,
        records: Vec<Row>,
    },
    /// A record held is replaced where it is, keeping the fields named in `keep` of the copy held; a new one goes
    /// before `before`, or last when none.
    Put {
        slot: Slot,
        id: Key,
        before: Option<Key>,
        keep: Vec<String>,
        record: Value,
    },
    /// These fields of a record held take these values.
    Merge {
        slot: Slot,
        id: Key,
        fields: Value,
    },
    /// A streamed chunk: the text of the event the record holds (`EventRead`'s `event`) grows by it.
    Append {
        slot: Slot,
        id: Key,
        text: String,
    },
    Drop {
        slot: Slot,
        id: Key,
    },
    /// The model is gone: a card's events once its chain is opened again or closed.
    Forget {
        slot: Slot,
    },
}

impl Patch {
    fn slot(&self) -> &Slot {
        match self {
            Self::Reset { slot, .. }
            | Self::Put { slot, .. }
            | Self::Merge { slot, .. }
            | Self::Append { slot, .. }
            | Self::Drop { slot, .. }
            | Self::Forget { slot } => slot,
        }
    }
}

/// What applying a patch means for a tab that shows its model.
enum Applied {
    Nothing,
    Whole,
    Changed(Change),
}

type Rows = IndexMap<Key, Value>;

/// The models a tab shows, with the freshness it was last told of each; none until it is told the model whole.
type Shown = BTreeMap<Model, Option<Freshness>>;

#[derive(Debug, Default)]
pub(crate) struct Models {
    held: HashMap<Slot, Rows>,
    shown: BTreeMap<Tab, Shown>,
    /// The chain each tab shows: the one whose view and events it reads.
    chains: BTreeMap<Tab, Key>,
}

/// What a step tells the tabs, each tab's in the order it was said.
#[derive(Debug, Default)]
struct Telling {
    told: Vec<(Tab, Told)>,
    /// The row changes not told yet, by tab: told as one batch, before anything else that tab is told.
    changes: BTreeMap<Tab, Vec<Change>>,
}

impl Telling {
    fn change(&mut self, tab: Tab, change: Change) {
        self.changes.entry(tab).or_default().push(change);
    }

    fn tell(&mut self, tab: Tab, told: Told) {
        self.flush(tab);
        self.told.push((tab, told));
    }

    fn flush(&mut self, tab: Tab) {
        if let Some(changes) = self.changes.remove(&tab) {
            self.told.push((tab, Told::Rows { changes }));
        }
    }

    fn done(mut self) -> Vec<(Tab, Told)> {
        let tabs: Vec<Tab> = self.changes.keys().copied().collect();
        for tab in tabs {
            self.flush(tab);
        }
        self.told
    }
}

impl Models {
    /// Tab `tab` draws `model`: it is told it whole at the end of this step, even if it was shown already.
    pub(crate) fn show(&mut self, tab: Tab, model: Model) {
        self.shown.entry(tab).or_default().insert(model, None);
    }

    pub(crate) fn hide(&mut self, tab: Tab, model: &Model) {
        if let Some(shown) = self.shown.get_mut(&tab) {
            shown.remove(model);
        }
    }

    /// The tab is gone: it shows nothing, and what was kept for it alone goes.
    pub(crate) fn leave(&mut self, tab: Tab) {
        self.shown.remove(&tab);
        self.chains.remove(&tab);
        self.held.retain(|slot, _| slot.owner != Owner::Tab(tab));
    }

    /// Tab `tab` shows `chain` (none: no chain): when that changes, it is told each model of its chain whole at the end
    /// of this step, as the chain's records are held already.
    pub(crate) fn show_chain(&mut self, tab: Tab, chain: Option<&str>) {
        if self.chains.get(&tab).map(String::as_str) == chain {
            return;
        }
        match chain {
            Some(chain) => self.chains.insert(tab, chain.to_owned()),
            None => self.chains.remove(&tab),
        };
        if let Some(shown) = self.shown.get_mut(&tab) {
            for (model, told) in shown.iter_mut() {
                if sharing(model) == Sharing::Chain {
                    *told = None;
                }
            }
        }
    }

    /// The records a slot holds, in order.
    pub(crate) fn rows(&self, slot: &Slot) -> impl Iterator<Item = (&Key, &Value)> {
        self.held.get(slot).into_iter().flat_map(IndexMap::iter)
    }

    /// The slot tab `tab` reads `model` from; a tab that shows no chain reads its models from a slot of its own, which
    /// holds nothing.
    fn read_by(&self, tab: Tab, model: Model) -> Slot {
        let owner = match sharing(&model) {
            Sharing::Every => Owner::Every,
            Sharing::Tab => Owner::Tab(tab),
            Sharing::Chain => self
                .chains
                .get(&tab)
                .map_or(Owner::Tab(tab), |c| Owner::Chain(c.clone())),
        };
        Slot { owner, model }
    }

    /// Whether `tab` is told of what changes in `slot`.
    fn told_to(&self, slot: &Slot, tab: Tab) -> bool {
        match &slot.owner {
            Owner::Every => true,
            Owner::Tab(own) => *own == tab,
            Owner::Chain(chain) => self.chains.get(&tab) == Some(chain),
        }
    }

    /// Applies a step's patches, in order, and says what each tab is told: the changes to the rows of the models it
    /// shows, each model shown or read again whole, then each freshness that moved (`fresh`, the core's word for a
    /// tab's model now).
    pub(crate) fn apply(&mut self, patches: Vec<Patch>, fresh: impl Fn(Tab, &Model) -> Freshness) -> Vec<(Tab, Told)> {
        let mut telling = Telling::default();
        for patch in patches {
            let slot = patch.slot().clone();
            // The tabs told this slot's model whole, with how fresh each last heard it was.
            let told: Vec<(Tab, Freshness)> = self
                .shown
                .iter()
                .filter(|(tab, _)| self.told_to(&slot, **tab))
                .filter_map(|(tab, shown)| shown.get(&slot.model).copied().flatten().map(|f| (*tab, f)))
                .collect();
            match self.patch(patch, !told.is_empty()) {
                Applied::Nothing => {}
                Applied::Changed(change) => {
                    for (tab, _) in told {
                        telling.change(tab, change.clone());
                    }
                }
                Applied::Whole => {
                    for (tab, status) in told {
                        let whole = self.whole(&slot, status);
                        telling.tell(tab, whole);
                    }
                }
            }
        }
        let mut wholes = Vec::new();
        let mut moved = Vec::new();
        for (tab, shown) in &mut self.shown {
            for (model, told) in shown.iter_mut() {
                let now = fresh(*tab, model);
                match told {
                    None => wholes.push((*tab, model.clone(), now)),
                    Some(was) if *was != now => moved.push((*tab, model.clone(), now)),
                    Some(_) => continue,
                }
                *told = Some(now);
            }
        }
        for (tab, model, status) in wholes {
            let whole = self.whole(&self.read_by(tab, model), status);
            telling.tell(tab, whole);
        }
        for (tab, model, status) in moved {
            telling.tell(tab, Told::Status { model, status });
        }
        telling.done()
    }

    fn whole(&self, slot: &Slot, status: Freshness) -> Told {
        let rows = self
            .held
            .get(slot)
            .map(|rows| {
                rows.iter()
                    .map(|(id, record)| Row {
                        id: id.clone(),
                        record: record.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Told::Model {
            model: slot.model.clone(),
            rows,
            status,
        }
    }

    /// Applies one patch; what changed is said only for a model a tab has been told whole (`told`).
    fn patch(&mut self, patch: Patch, told: bool) -> Applied {
        match patch {
            Patch::Reset { slot, records } => {
                let rows = records.into_iter().map(|r| (r.id, r.record)).collect();
                self.held.insert(slot, rows);
                Applied::Whole
            }
            Patch::Forget { slot } => {
                self.held.remove(&slot);
                Applied::Whole
            }
            Patch::Put {
                slot,
                id,
                before,
                keep,
                mut record,
            } => {
                let rows = self.held.entry(slot.clone()).or_default();
                let (at, before) = match rows.get_full(&id) {
                    Some((at, _, old)) => {
                        keep_fields(&mut record, old, &keep);
                        (at, None)
                    }
                    None => match before.as_ref().and_then(|b| rows.get_index_of(b)) {
                        Some(at) => (at, before),
                        None => (rows.len(), None),
                    },
                };
                let said = told.then(|| record.clone());
                rows.shift_insert(at, id.clone(), record);
                said.map_or(Applied::Nothing, |record| {
                    Applied::Changed(Change::Put {
                        model: slot.model,
                        id,
                        before,
                        record,
                    })
                })
            }
            Patch::Merge { slot, id, fields } => {
                let Some(held) = self.held.get_mut(&slot).and_then(|rows| rows.get_mut(&id)) else {
                    return Applied::Nothing;
                };
                if let (Some(held), Value::Object(fields)) = (held.as_object_mut(), fields) {
                    held.extend(fields);
                }
                if !told {
                    return Applied::Nothing;
                }
                Applied::Changed(Change::Put {
                    model: slot.model,
                    id,
                    before: None,
                    record: held.clone(),
                })
            }
            Patch::Append { slot, id, text } => {
                let Some(held) = self
                    .held
                    .get_mut(&slot)
                    .and_then(|rows| rows.get_mut(&id))
                    .and_then(|read| read.get_mut("event"))
                    .and_then(Value::as_object_mut)
                else {
                    return Applied::Nothing;
                };
                match held.get_mut("text") {
                    Some(Value::String(grown)) => grown.push_str(&text),
                    _ => {
                        held.insert("text".to_owned(), Value::String(text.clone()));
                    }
                }
                if told {
                    Applied::Changed(Change::Append {
                        model: slot.model,
                        id,
                        text,
                    })
                } else {
                    Applied::Nothing
                }
            }
            Patch::Drop { slot, id } => {
                let gone = self
                    .held
                    .get_mut(&slot)
                    .and_then(|rows| rows.shift_remove(&id))
                    .is_some();
                if gone && told {
                    Applied::Changed(Change::Drop { model: slot.model, id })
                } else {
                    Applied::Nothing
                }
            }
        }
    }
}

/// The fields named in `keep` take their values in the copy held.
fn keep_fields(record: &mut Value, held: &Value, keep: &[String]) {
    let (Some(record), Some(held)) = (record.as_object_mut(), held.as_object()) else {
        return;
    };
    for field in keep {
        if let Some(value) = held.get(field) {
            record.insert(field.clone(), value.clone());
        }
    }
}
