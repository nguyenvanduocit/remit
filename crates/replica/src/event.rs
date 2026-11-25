//! The replica of the event open on screen (`openEvent.ts`): what the server answered (what arrived and the firings it
//! caused) and the pushes that change it afterwards. Opening events one after another ends on the last one asked for;
//! a push for the event while it is being opened waits for its answer.

use link::Failure;
use serde_json::Value;
use wire::app::{EventRecord, client};

use crate::calls::{Calls, Purpose};
use crate::index::Index;
use crate::io::Tab;
use crate::records::{Out, read};
use crate::shell::{Key, Model};

const ONE: &str = "";

#[derive(Debug)]
pub(crate) struct OpenEvent {
    /// The tab that opened it: its model is kept, and its answer read, for that tab alone.
    tab: Tab,
    shown: Option<Key>,
    pub(crate) opening: Key,
    seq: u64,
    /// The event last asked to open: what a reconnect opens again.
    wanted: Key,
    by_sync: Option<u64>,
    early: Vec<(EventRecord, Value)>,
    event: Index<()>,
}

impl OpenEvent {
    /// What tab `tab` opens; nothing yet.
    pub(crate) fn new(tab: Tab) -> Self {
        Self {
            tab,
            shown: None,
            opening: Key::new(),
            seq: 0,
            wanted: Key::new(),
            by_sync: None,
            early: Vec::new(),
            event: Index::of(tab, Model::OpenEvent),
        }
    }
}

impl OpenEvent {
    pub(crate) fn open(&mut self, calls: &mut Calls, id: Key, by_sync: Option<u64>) {
        self.seq = self.seq.wrapping_add(1);
        self.wanted.clone_from(&id);
        self.opening.clone_from(&id);
        self.by_sync = by_sync;
        self.early.clear();
        calls.ask(
            client::event_get(id),
            Purpose::EventGet {
                tab: self.tab,
                seq: self.seq,
            },
        );
    }

    /// The sync attempt an opening under way is part of, when a reconnect asked for it.
    pub(crate) fn by_sync(&self) -> Option<u64> {
        self.by_sync
    }

    /// After a reconnect, once the snapshot is in: the event last asked for is opened again. Whether it was.
    pub(crate) fn reload(&mut self, calls: &mut Calls, sync: u64) -> bool {
        if self.wanted.is_empty() {
            return false;
        }
        self.open(calls, self.wanted.clone(), Some(sync));
        true
    }

    /// Its page is gone: nothing of the event stays held, and an opening on its way is dropped.
    pub(crate) fn close(&mut self, out: &mut Out) {
        self.seq = self.seq.wrapping_add(1);
        self.wanted.clear();
        self.opening.clear();
        self.shown = None;
        self.early.clear();
        self.event.reset(Vec::new(), &mut out.patches);
    }

    /// The opening's answer, with the sync that asked for it; none for one a newer opening replaced.
    pub(crate) fn answered(
        &mut self,
        seq: u64,
        result: Result<Value, Failure>,
        out: &mut Out,
    ) -> Option<(Option<u64>, Result<(), Failure>)> {
        if seq != self.seq {
            return None;
        }
        let by_sync = self.by_sync.take();
        self.opening.clear();
        let early = std::mem::take(&mut self.early);
        let event = result.and_then(|json| Ok((read::<EventRecord>(&json)?, json)));
        match event {
            Ok((event, json)) => {
                self.shown = Some(event.id);
                self.event.reset(vec![(ONE.to_owned(), (), json)], &mut out.patches);
                for (e, record) in early {
                    self.put(&e, record, out);
                }
                Some((by_sync, Ok(())))
            }
            Err(failure) => Some((by_sync, Err(failure))),
        }
    }

    /// What the server pushed of an event: the open one takes it, keeping what arrived (a push carries none of it); one
    /// being opened holds it until the answer is in. The push that arrives last is the newer.
    pub(crate) fn put(&mut self, e: &EventRecord, record: Value, out: &mut Out) {
        if self.opening == e.id {
            self.early.push((e.clone(), record));
        } else if self.shown.as_deref() == Some(e.id.as_str()) {
            self.event.put_keeping(
                ONE.to_owned(),
                (),
                record,
                None,
                vec!["event".to_owned()],
                &mut out.patches,
            );
        }
    }
}
