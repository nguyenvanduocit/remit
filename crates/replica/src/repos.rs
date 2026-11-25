//! The repositories a tab shows (ADR-0114, ADR-0116), each as the server lays it out (`RepoView`): its queues, its
//! newest releases with the landings each carried, and its last release pause. The snapshot leaves them out: a tab reads
//! the one it opens (the Releases view, `repo_get`), or every one (the merge queues, `repo_views`); `repo_updated`
//! replaces a view whole, and a new snapshot reads them again. A view pushed while the read is on its way is put over
//! the answer once it comes. A view carries no revision: every write of a repository pushes its whole view, computed
//! after the write and pushed in write order, so the last push is the newest and pushes applied in arrival order over
//! the answer converge on it.

use link::{ErrorCode, Failure};
use serde_json::Value;
use wire::app::{Push, RepoView, client};

use crate::calls::{Calls, Purpose};
use crate::index::Index;
use crate::io::Tab;
use crate::records::{Out, read};
use crate::shell::{Key, Model, Notice, What};

/// The key of the one repository open, so the page reads it as the model's one record.
const ONE: &str = "";

/// Which repositories a tab reads.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Wants {
    Nothing,
    /// The one open, by its catalog id.
    One(Key),
    Every,
}

#[derive(Debug)]
pub(crate) struct Repos {
    /// The tab that reads them: its models are kept for that tab alone.
    tab: Tab,
    /// Reads every repository (`Model::Repos`), else the one open (`Model::Repo`).
    every: bool,
    /// The views, by repository (`Every`), or the one open under `ONE`.
    views: Index<()>,
    wants: Wants,
    /// The answer came since they were opened or the last snapshot.
    pub(crate) read: bool,
    /// Counts the reads: an answer asked for before the last lands after it, and is dropped.
    generation: u64,
    /// Views pushed before the answer, by repository, in the order they came.
    early: Vec<(Key, Value)>,
}

impl Repos {
    /// The open repository's view of tab `tab` (`Model::Repo`); nothing open yet.
    pub(crate) fn one(tab: Tab) -> Self {
        Self::new(tab, false)
    }

    /// Every repository's view of tab `tab` (`Model::Repos`); not read yet.
    pub(crate) fn every(tab: Tab) -> Self {
        Self::new(tab, true)
    }

    fn new(tab: Tab, every: bool) -> Self {
        Self {
            tab,
            every,
            views: Index::of(tab, if every { Model::Repos } else { Model::Repo }),
            wants: Wants::Nothing,
            read: false,
            generation: 0,
            early: Vec::new(),
        }
    }

    /// Opens repository `repo`; empty closes it. The one open already is kept as it is.
    pub(crate) fn open_one(&mut self, calls: &mut Calls, repo: Key, out: &mut Out) {
        let wants = if repo.is_empty() {
            Wants::Nothing
        } else {
            Wants::One(repo)
        };
        self.want(calls, wants, out);
    }

    /// Opens every repository, or closes them.
    pub(crate) fn open_every(&mut self, calls: &mut Calls, open: bool, out: &mut Out) {
        self.want(calls, if open { Wants::Every } else { Wants::Nothing }, out);
    }

    fn want(&mut self, calls: &mut Calls, wants: Wants, out: &mut Out) {
        if wants != self.wants {
            self.wants = wants;
            self.read_again(calls, out);
        }
    }

    /// What was read is of the server as it was: a new snapshot reads what is open again.
    pub(crate) fn read_again(&mut self, calls: &mut Calls, out: &mut Out) {
        self.views.reset(Vec::new(), &mut out.patches);
        self.read = false;
        self.early.clear();
        self.generation = self.generation.wrapping_add(1);
        let purpose = Purpose::Repos {
            tab: self.tab,
            every: self.every,
            generation: self.generation,
        };
        match &self.wants {
            Wants::Nothing => {}
            Wants::One(repo) => calls.ask(client::repo_get(repo.clone()), purpose),
            Wants::Every => calls.ask(client::repo_views(), purpose),
        }
    }

    pub(crate) fn answered(&mut self, generation: u64, result: Result<Value, Failure>, out: &mut Out) {
        if generation != self.generation {
            return;
        }
        match result.and_then(|json| self.views_of(json)) {
            Ok(views) => {
                self.views.reset(views, &mut out.patches);
                self.read = true;
                for (repo, record) in std::mem::take(&mut self.early) {
                    self.put(&repo, record, out);
                }
            }
            // With no session the banner says so already, and the next snapshot reads them again.
            Err(failure) if failure.code == ErrorCode::Disconnected => {}
            Err(failure) => out.notice_to(
                self.tab,
                Notice::CouldNot {
                    what: What::ReadRepos,
                    failure,
                },
            ),
        }
    }

    /// The records of an answer, each under its key, in the server's order.
    fn views_of(&self, json: Value) -> Result<Vec<(Key, (), Value)>, Failure> {
        if self.wants == Wants::Every {
            let views: Vec<RepoView> = read(&json)?;
            let records = match json {
                Value::Array(items) => items,
                _ => Vec::new(),
            };
            return Ok(views.into_iter().zip(records).map(|(v, r)| (v.repo, (), r)).collect());
        }
        read::<RepoView>(&json)?;
        Ok(vec![(ONE.to_owned(), (), json)])
    }

    /// A repository's view, as pushed whole.
    pub(crate) fn on_push(&mut self, push: &Push, record: &Value, out: &mut Out) {
        let Push::repo_updated { view } = push else {
            return;
        };
        if self.read {
            self.put(&view.repo, record.clone(), out);
        } else if self.wants != Wants::Nothing {
            self.early.push((view.repo.clone(), record.clone()));
        }
    }

    /// A repository wanted takes its view; a new one goes last.
    fn put(&mut self, repo: &str, record: Value, out: &mut Out) {
        let key = match &self.wants {
            Wants::One(open) if open == repo => ONE,
            Wants::Every => repo,
            _ => return,
        };
        self.views.put(key.to_owned(), (), record, None, &mut out.patches);
    }
}

#[cfg(test)]
mod tests;
