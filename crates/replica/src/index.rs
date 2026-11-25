//! What a rule keeps of a model: each record's key, in the model's order, with the few facts the rule reads (its
//! `rev`, and what a rule needs). Every change here is said as a `Patch`, which `models.rs` applies to the records.

use std::collections::HashMap;

use serde_json::Value;

use crate::io::Tab;
use crate::models::{Owner, Patch, Slot};
use crate::shell::{Key, Model, Row};

/// Of two copies of one record, the one at the higher revision is the newer: the server counts each stored change of a
/// record in its `rev`, so a copy that arrives late never overwrites a newer one.
pub(crate) fn newer(held: Option<i64>, arrived: i64) -> bool {
    held.is_none_or(|rev| arrived > rev)
}

#[derive(Debug)]
pub(crate) struct Index<M> {
    slot: Slot,
    order: Vec<Key>,
    meta: HashMap<Key, M>,
}

impl<M> Index<M> {
    /// A model every tab shares.
    pub(crate) fn new(model: Model) -> Self {
        Self::kept(Slot {
            owner: Owner::Every,
            model,
        })
    }

    /// A model of what `tab` opened, kept for it alone.
    pub(crate) fn of(tab: Tab, model: Model) -> Self {
        Self::kept(Slot {
            owner: Owner::Tab(tab),
            model,
        })
    }

    /// A model of chain `chain`, kept once for every tab that shows it.
    pub(crate) fn of_chain(chain: &str, model: Model) -> Self {
        Self::kept(Slot {
            owner: Owner::Chain(chain.to_owned()),
            model,
        })
    }

    fn kept(slot: Slot) -> Self {
        Self {
            slot,
            order: Vec::new(),
            meta: HashMap::new(),
        }
    }

    pub(crate) fn slot(&self) -> &Slot {
        &self.slot
    }

    pub(crate) fn get(&self, key: &str) -> Option<&M> {
        self.meta.get(key)
    }

    pub(crate) fn has(&self, key: &str) -> bool {
        self.meta.contains_key(key)
    }

    pub(crate) fn first(&self) -> Option<&Key> {
        self.order.first()
    }

    pub(crate) fn keys(&self) -> impl Iterator<Item = (&Key, &M)> {
        self.order.iter().filter_map(|k| self.meta.get(k).map(|m| (k, m)))
    }

    /// The store holds these, in this order.
    pub(crate) fn reset(&mut self, records: Vec<(Key, M, Value)>, out: &mut Vec<Patch>) {
        self.order.clear();
        self.meta.clear();
        let mut entries = Vec::with_capacity(records.len());
        for (id, meta, record) in records {
            if self.meta.insert(id.clone(), meta).is_none() {
                self.order.push(id.clone());
            }
            entries.push(Row { id, record });
        }
        out.push(Patch::Reset {
            slot: self.slot.clone(),
            records: entries,
        });
    }

    /// A record held is replaced where it is; a new one goes before `before` (none: last).
    pub(crate) fn put(&mut self, id: Key, meta: M, record: Value, before: Option<Key>, out: &mut Vec<Patch>) {
        self.put_keeping(id, meta, record, before, Vec::new(), out);
    }

    pub(crate) fn put_keeping(
        &mut self,
        id: Key,
        meta: M,
        record: Value,
        before: Option<Key>,
        keep: Vec<String>,
        out: &mut Vec<Patch>,
    ) {
        let before = if self.meta.contains_key(&id) {
            None
        } else {
            let at = before
                .as_ref()
                .and_then(|b| self.position(b))
                .unwrap_or(self.order.len());
            self.order.insert(at, id.clone());
            before.filter(|b| self.meta.contains_key(b))
        };
        self.meta.insert(id.clone(), meta);
        out.push(Patch::Put {
            slot: self.slot.clone(),
            id,
            before,
            keep,
            record,
        });
    }

    /// The facts of a record whose body changes by a `Merge` or an `Append` the caller says.
    pub(crate) fn meta_mut(&mut self, key: &str) -> Option<&mut M> {
        self.meta.get_mut(key)
    }

    pub(crate) fn remove(&mut self, key: &str, out: &mut Vec<Patch>) -> Option<M> {
        let meta = self.meta.remove(key)?;
        if let Some(at) = self.position(key) {
            self.order.remove(at);
        }
        out.push(Patch::Drop {
            slot: self.slot.clone(),
            id: key.to_owned(),
        });
        Some(meta)
    }

    /// Lets go of what lies beyond the first `n`.
    pub(crate) fn truncate(&mut self, n: usize, out: &mut Vec<Patch>) {
        let beyond = self.order.split_off(n.min(self.order.len()));
        for key in beyond {
            self.meta.remove(&key);
            out.push(Patch::Drop {
                slot: self.slot.clone(),
                id: key,
            });
        }
    }

    /// Lets go of the oldest, those before the last `n`. Whether any went.
    pub(crate) fn keep_last(&mut self, n: usize, out: &mut Vec<Patch>) -> bool {
        let over = self.order.len().saturating_sub(n);
        for key in self.order.drain(..over) {
            self.meta.remove(&key);
            out.push(Patch::Drop {
                slot: self.slot.clone(),
                id: key,
            });
        }
        over > 0
    }

    /// The key a record would go before to keep the store ordered by `rank` (lowest first; ties go after); none: last.
    pub(crate) fn before_by<R: Ord>(&self, rank: impl Fn(&Key, &M) -> R, new: &R) -> Option<Key> {
        self.keys().find(|(k, m)| rank(k, m) > *new).map(|(k, _)| k.clone())
    }

    /// Searched from the end: what changes is mostly recent.
    fn position(&self, key: &str) -> Option<usize> {
        self.order.iter().rposition(|k| k == key)
    }
}
