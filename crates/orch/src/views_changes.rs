//! A chain's changes as they are now, read from its worktrees with git, for the views that judge its verdicts against
//! them (views.rs). Git may take seconds where the store takes microseconds, so it is read apart from the views'
//! computation: a chain's views read its changes again before they take the views' lock (`read`), and the computation
//! only looks up what was last read (`current`). No view waits on git while it holds that lock, and a channel that loads
//! the snapshot runs no git at all.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use model::{Approval, Chain, ChainStatus, RepoWork};

use chains::git::git;
use shipping::gate::patch_id;
use shipping::verification::Changes;

/// How long reading a chain's changes may take.
const CHANGES_READ_LIMIT: Duration = Duration::from_secs(30);

/// A worktree's change as last read: its patch id (`change`) when HEAD and the base's tip were `at`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Head {
    at: String,
    change: String,
}

/// The changes last read of every chain's worktrees.
#[derive(Default)]
pub(crate) struct ChangesRead {
    /// Held while git reads, so that reads of a worktree land in the order they were made: a slow read never puts back
    /// what a later one found.
    reading: tokio::sync::Mutex<()>,
    /// By worktree: the change last read there.
    heads: Mutex<HashMap<String, Head>>,
}

/// Whether chain c's change is read: only while agents may still move it (the chain is not done) and a reviewer or QC
/// has judged one.
fn judged(c: &Chain, verdicts: &[Approval]) -> bool {
    *c.status() != ChainStatus::Done && verdicts.iter().any(|a| a.role.gate().is_some())
}

/// Chain c's worktrees: one with no directory is left out, as git would read the directory the server runs in.
fn worktrees(c: &Chain) -> impl Iterator<Item = &RepoWork> {
    c.repos.iter().filter(|rw| !rw.path.is_empty())
}

impl ChangesRead {
    fn heads(&self) -> MutexGuard<'_, HashMap<String, Head>> {
        self.heads.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Reads chain c's changes again from its worktrees, when its verdicts are judged against them (`judged`). A
    /// worktree's patch id is computed again only when its HEAD or its base's tip moved; one that cannot be read is
    /// forgotten, so its verdicts are not checked against a change it may no longer hold.
    pub(crate) async fn read(&self, c: &Chain, verdicts: &[Approval]) {
        if !judged(c, verdicts) {
            return;
        }
        let _one = self.reading.lock().await;
        let now = tokio::time::Instant::now();
        let deadline = now.checked_add(CHANGES_READ_LIMIT).unwrap_or(now);
        for rw in worktrees(c) {
            let tip = format!("origin/{}", rw.base);
            let at = tokio::time::timeout_at(deadline, git(&rw.path, &["rev-parse", "HEAD", &tip]))
                .await
                .unwrap_or_else(|_| Err("timed out".to_owned()));
            if let Ok(at) = &at
                && self.heads().get(&rw.path).is_some_and(|h| h.at == *at)
            {
                continue;
            }
            let read = match at {
                Ok(at) => tokio::time::timeout_at(deadline, patch_id(&rw.path, &tip, "HEAD"))
                    .await
                    .unwrap_or_else(|_| Err("timed out".to_owned()))
                    .map(|change| Head { at, change }),
                Err(err) => Err(err),
            };
            match read {
                Ok(h) => {
                    self.heads().insert(rw.path.clone(), h);
                }
                Err(err) => {
                    self.heads().remove(&rw.path);
                    tracing::warn!(chain = %c.id, repo = %rw.repo, %err, "change not read: its verdicts are not checked against it");
                }
            }
        }
    }

    /// Chain c's changes as last read, by repository, for the views that judge verdicts, its approvals, against them:
    /// none when they are not judged (`judged`), and a repository whose change was never read is left out.
    pub(crate) fn current(&self, c: &Chain, verdicts: &[Approval]) -> Changes {
        if !judged(c, verdicts) {
            return Changes::new();
        }
        let heads = self.heads();
        worktrees(c)
            .filter_map(|rw| heads.get(&rw.path).map(|h| (rw.repo.clone(), h.change.clone())))
            .collect()
    }

    /// Forgets the changes of every worktree but those of chains, the chains still on the board.
    pub(crate) fn keep_only(&self, chains: &[Chain]) {
        let kept: HashSet<&str> = chains.iter().flat_map(worktrees).map(|rw| rw.path.as_str()).collect();
        self.heads().retain(|path, _| kept.contains(path.as_str()));
    }
}
