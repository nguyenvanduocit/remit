//! The archived chains, which the snapshot leaves out however many the owner put away (`archived.ts`): read a page at
//! a time as he looks at them, and held until the snapshot is loaded again. The shell lays the pages over the catalog's
//! chains, each by its latest copy.

use link::{ErrorCode, Failure};
use serde_json::Value;
use wire::app::{ChainPage, client};

use crate::calls::{Calls, Purpose};
use crate::catalog::Catalog;
use crate::index::{Index, newer};
use crate::io::Tab;
use crate::records::{Out, read, take_list};
use crate::shell::{Model, Notice, What};

#[derive(Debug)]
pub(crate) struct Archived {
    /// The tab that opened it: its model is kept for that tab alone.
    tab: Tab,
    pages: Index<i64>,
    next: String,
    pub(crate) read: bool,
    reading: bool,
    /// Counts the snapshots that forgot the pages: a page asked for before one lands after it, and is dropped.
    pub(crate) generation: u64,
}

impl Archived {
    /// What tab `tab` opens; nothing yet.
    pub(crate) fn new(tab: Tab) -> Self {
        Self {
            tab,
            pages: Index::of(tab, Model::Archived),
            next: String::new(),
            read: false,
            reading: false,
            generation: 0,
        }
    }
}

impl Archived {
    /// Whether a page after those read is there to read.
    pub(crate) fn more(&self) -> bool {
        !self.read || !self.next.is_empty()
    }

    /// Reads the next page: the first, or the one after the pages read.
    pub(crate) fn read_next(&mut self, calls: &mut Calls) {
        if self.reading || !self.more() {
            return;
        }
        self.reading = true;
        let purpose = Purpose::Archived {
            tab: self.tab,
            generation: self.generation,
        };
        calls.ask(client::chains_archived(self.next.clone()), purpose);
    }

    pub(crate) fn answered(
        &mut self,
        generation: u64,
        result: Result<Value, Failure>,
        catalog: &mut Catalog,
        out: &mut Out,
    ) {
        if generation != self.generation {
            return;
        }
        self.reading = false;
        let page = result.and_then(|json| Ok((read::<ChainPage>(&json)?, json)));
        match page {
            Ok((page, mut json)) => {
                for (c, v) in page.chains.iter().zip(take_list(&mut json, "chains")) {
                    if newer(self.pages.get(&c.chain.id).copied(), c.chain.rev) {
                        self.pages
                            .put(c.chain.id.clone(), c.chain.rev, v, None, &mut out.patches);
                    }
                }
                for (s, v) in page.standings.iter().zip(take_list(&mut json, "standings")) {
                    catalog.put_standing(s, v, out);
                }
                self.next = page.next;
                self.read = true;
            }
            // With no session the banner says so already, and the snapshot of the next one forgets the pages, which has
            // the board ask for them again.
            Err(failure) if failure.code == ErrorCode::Disconnected => {}
            Err(failure) => out.notice_to(
                self.tab,
                Notice::CouldNot {
                    what: What::ReadArchived,
                    failure,
                },
            ),
        }
    }

    /// The pages read are of the server as it was: a new snapshot starts them again.
    pub(crate) fn forget(&mut self, out: &mut Out) {
        self.pages.reset(Vec::new(), &mut out.patches);
        self.next.clear();
        self.read = false;
        self.reading = false;
        self.generation = self.generation.wrapping_add(1);
    }
}
