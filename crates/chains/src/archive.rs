//! Archiving and unarchiving a chain, and collecting the worktrees of finished chains: a worktree unchanged for its
//! keep and with no landing on its way is removed, and unarchiving a chain brings its worktree back (`COLLECT_EVERY`
//! in automation.rs times the collection).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use model::{Actor, Chain, ChainStatus, Cleanup, DecisionKind, RepoWork, Time, Worktree};

use crate::apply::Records;
use crate::chain_core::Core;
use crate::git::RepoRemote;
use crate::git::ensure_clone;
use crate::git::git;
use crate::git::own_worktree;
use crate::git::path_str;
use crate::health::elapsed;
use crate::outbox::by_remit;
use crate::request::Service;
use crate::request::{Done, Request};
use crate::schedule::seconds_text;
use crate::step::{ArchiveCmd, ChainCmd, UnarchiveCmd};
use crate::{Error, Result};

/// How long removing a worktree may take.
const REMOVE_TIMEOUT: Duration = Duration::from_secs(60);

/// How long bringing a worktree back may take: its clone may fetch first.
const RESTORE_TIMEOUT: Duration = Duration::from_secs(180);

// A finished chain keeps its worktrees while the owner may still need them: until Config.keep_worktrees after it last
// changed, and for as long as a landing of it is on its way or a submit is in progress. Then the scheduler removes them
// (collect_worktrees), each only while it is the chain's own worktree, and the chain's decisions say so. The branches stay
// in their clones, so no committed work is lost, and reopening the chain brings its worktrees back (restore_worktrees).
// Without it, chains started every hour fill the disk with checkouts and build caches in weeks.

/// A worktree of a finished chain to remove.
#[derive(Debug, Clone, PartialEq)]
struct Garbage {
    chain_id: String,
    repo: String,
    w: Worktree,
}

/// The worktrees to remove by now (a folder worked in place is the user's, never removed): those of finished chains (archived or not) unchanged for keep, with no landing on its
/// way (`landing`, the ids of chains with one) and no submit in progress, except a path another chain still names. None
/// when keep is zero.
fn collectible(chains: &[Chain], landing: &HashSet<String>, now: Time, keep: Duration) -> Vec<Garbage> {
    if keep.is_zero() {
        return Vec::new();
    }
    let done = |c: &Chain| {
        *c.status() == ChainStatus::Done
            && elapsed(c.updated_at, now) >= keep
            && !landing.contains(&c.id)
            && !c.repos.iter().any(|rw| rw.submit.is_some())
    };
    let kept: HashSet<PathBuf> = chains
        .iter()
        .filter(|c| !done(c))
        .flat_map(|c| c.repos.iter().filter_map(RepoWork::worktree).map(|w| clean(&w.path)))
        .collect();
    chains
        .iter()
        .filter(|c| done(c))
        .flat_map(|c| {
            c.repos.iter().filter_map(|rw| {
                rw.worktree().map(|w| Garbage {
                    chain_id: c.id.clone(),
                    repo: rw.repo.clone(),
                    w: w.clone(),
                })
            })
        })
        .filter(|g| !kept.contains(&clean(&g.w.path)))
        .collect()
}

/// path with its `.` elements and trailing separator dropped, so two paths written alike compare equal.
fn clean(path: &str) -> PathBuf {
    Path::new(path).components().collect()
}

/// Whether anything is at path; one that cannot be looked at counts as there, so nothing is put over it.
async fn exists(path: &str) -> bool {
    !matches!(tokio::fs::metadata(path).await, Err(err) if err.kind() == std::io::ErrorKind::NotFound)
}

/// Detaches a chain's worktree from its clone and deletes it, only while it is the chain's own: a worktree on another
/// branch may be another chain's or the owner's. One already gone is not an error.
async fn remove_worktree(w: &Worktree) -> Result<(), String> {
    if !exists(&w.path).await {
        return Ok(());
    }
    tokio::time::timeout(REMOVE_TIMEOUT, async {
        own_worktree(w).await?;
        let common = git(&w.path, &["rev-parse", "--path-format=absolute", "--git-common-dir"]).await?;
        git(&common, &["worktree", "remove", "--force", &w.path])
            .await
            .map(|_| ())
    })
    .await
    .map_err(|_| format!("removing {} took over {}", w.path, seconds_text(REMOVE_TIMEOUT)))?
}

impl Core {
    /// Puts a finished chain away for why (empty allowed), recorded on its decision: the lists leave it out, nothing else
    /// changes. Its landings still land, and a message from the owner reopens it, which brings it back.
    pub fn archive(
        self: &Arc<Self>,
        req: Request,
        chain_id: String,
        why: String,
    ) -> impl std::future::Future<Output = Result<Done>> + Send + use<> {
        let by = req.by.clone();
        self.act_on_chain(req, "Archive", chain_id, move |o, id| async move {
            o.archive_as(&id, why, by).await
        })
    }

    /// Archives the finished chain for why as by (the owner, or his Assistant on his words).
    pub async fn archive_as(&self, chain_id: &str, why: String, by: Actor) -> Result<()> {
        self.apply(chain_id, ChainCmd::Archive(ArchiveCmd { why, by }), Records::default())
            .await
            .map(|_| ())
    }

    /// Brings an archived chain back into the lists for why (empty allowed), recorded on its decision.
    pub fn unarchive(
        self: &Arc<Self>,
        req: Request,
        chain_id: String,
        why: String,
    ) -> impl std::future::Future<Output = Result<Done>> + Send + use<> {
        let by = req.by.clone();
        self.act_on_chain(req, "Unarchive", chain_id, move |o, id| async move {
            o.unarchive_as(&id, why, by).await
        })
    }

    /// Brings the archived chain back for why as by (the owner, or his Assistant on his words).
    pub async fn unarchive_as(&self, chain_id: &str, why: String, by: Actor) -> Result<()> {
        self.apply(
            chain_id,
            ChainCmd::Unarchive(UnarchiveCmd { why, by }),
            Records::default(),
        )
        .await
        .map(|_| ())
    }

    /// Removes a finished chain for good: its records, its worktrees and, when Remit made it, its directory. What it
    /// landed stays on its base and its landings stay recorded, so no landed work is lost; what of it has not landed is
    /// withdrawn with it and never lands, but a change the queue is checking and pushing now refuses the delete until it
    /// has settled. The records go first, in one step
    /// with the check that the chain may go (so no reopen can come between) and with the list of files it leaves; the
    /// files follow. A crash before they are gone leaves that list, and Recover removes them.
    pub fn delete(
        self: &Arc<Self>,
        req: Request,
        chain_id: String,
    ) -> impl std::future::Future<Output = Result<Done>> + Send + use<> {
        self.act_on_chain(req, "Delete", chain_id, move |o, chain_id| async move {
            let id = chain_id.clone();
            let landing: crate::apply::Check = std::sync::Arc::new(move |tx| {
                let pushing = tx
                    .chain_landings(&id)?
                    .into_iter()
                    .find(|l| l.state == model::LandingState::Checking);
                match pushing {
                    Some(l) => Err(Error::refused(format!(
                        "its change to {} is being checked and pushed ({}): try again once it has landed or come back",
                        l.repo, l.id
                    ))),
                    None => Ok(()),
                }
            });
            let w = Records {
                checks: vec![landing],
                ..Records::default()
            };
            let a = o.apply(&chain_id, ChainCmd::Delete, w).await?;
            o.forget(&chain_id);
            o.clean_up(&o.cleanup_of(&a.chain))
                .await
                .map_err(|e| Error::refused(format!("the chain is deleted, but some of its files are left: {e}")))
        })
    }

    /// What deleting ch leaves on disk to remove: its worktrees and the directory Remit made for it (`workdir_of`), by
    /// that rule rather than the path the chain recorded: a chain may name a directory the owner chose, which stays.
    pub fn cleanup_of(&self, ch: &Chain) -> Cleanup {
        Cleanup {
            chain_id: ch.id.clone(),
            repos: ch.repos.clone(),
            dir: self.workdir_of(&ch.id),
        }
    }

    /// Removes what a deleted chain left on disk and then its record, and says what it could not remove. The chain's
    /// directory stays while anything in it was left.
    pub async fn clean_up(&self, c: &Cleanup) -> Result<(), String> {
        let mut left = Vec::new();
        for (rw, w) in c.repos.iter().filter_map(|rw| rw.worktree().map(|w| (rw, w))) {
            if let Err(err) = remove_worktree(w).await {
                left.push(format!("worktree of {} kept: {err}", rw.repo));
            }
        }
        if !c.dir.is_empty() && left.is_empty() {
            match tokio::fs::remove_dir_all(&c.dir).await {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => left.push(format!("remove {}: {err}", c.dir)),
            }
        }
        let id = c.chain_id.clone();
        if let Err(err) = self.store.update(move |tx| tx.delete_cleanup(&id)).await {
            left.push(err.to_string());
        }
        if left.is_empty() { Ok(()) } else { Err(left.join("; ")) }
    }

    /// Removes the worktrees `collectible` says go by now, and records each in its chain's decisions. One already gone is
    /// skipped; one that is not the chain's own worktree, or cannot be removed, stays, and the log says why.
    pub async fn collect_worktrees(&self, now: Time) {
        let read = self
            .store
            .read(|r| Ok::<_, store::Error>((r.chains()?, r.open_landings()?)))
            .await;
        let (chains, landing) = match read {
            Ok((chains, open)) => (
                chains,
                open.into_iter().map(|l| l.chain_id).collect::<HashSet<String>>(),
            ),
            Err(err) => {
                tracing::error!(%err, "collect worktrees");
                return;
            }
        };
        let keep = self.cfg.keep_worktrees;
        for g in collectible(&chains, &landing, now, keep) {
            if !exists(&g.w.path).await {
                continue;
            }
            if let Err(err) = remove_worktree(&g.w).await {
                tracing::warn!(chain = %g.chain_id, path = %g.w.path, %err, "worktree not collected");
                continue;
            }
            let text = format!("Remove the worktree of {} ({}).", g.repo, g.w.path);
            let why = format!(
                "The chain is finished, nothing of it waits to land, and it kept its worktree for {}. Its branch {} stays in the clone; reopening the chain brings the worktree back.",
                seconds_text(keep),
                g.w.branch
            );
            self.decide(&g.chain_id, DecisionKind::Repo, &text, &why, "", by_remit())
                .await;
        }
    }

    /// Brings back the worktrees of reopened chain ch that `collect_worktrees` removed, each on its branch, and records each
    /// in the chain's decisions, as it records one that cannot come back.
    pub async fn restore_worktrees(&self, ch: &Chain) {
        for (rw, w) in ch.repos.iter().filter_map(|rw| rw.worktree().map(|w| (rw, w))) {
            if exists(&w.path).await {
                continue;
            }
            let (text, why) = match self.restore_worktree(w).await {
                Ok(()) => (
                    format!("Bring back the worktree of {} on branch {}.", rw.repo, w.branch),
                    "The chain was reopened.".to_owned(),
                ),
                Err(err) => (
                    format!("Could not bring back the worktree of {} ({}).", rw.repo, w.path),
                    err,
                ),
            };
            self.decide(&ch.id, DecisionKind::Repo, &text, &why, "", by_remit())
                .await;
        }
    }

    /// Checks out rw's branch again at its path, from Remit's clone of the repository.
    async fn restore_worktree(&self, w: &Worktree) -> Result<(), String> {
        let clone = path_str(&self.clone_dir(&RepoRemote {
            host: w.host.clone(),
            project: w.project.clone(),
        }));
        tokio::time::timeout(RESTORE_TIMEOUT, async {
            ensure_clone(Path::new(&clone), "").await?;
            git(&clone, &["worktree", "prune"]).await?;
            git(&clone, &["worktree", "add", &w.path, &w.branch])
                .await
                .map(|_| ())
        })
        .await
        .map_err(|_| format!("bringing back {} took over {}", w.path, seconds_text(RESTORE_TIMEOUT)))?
    }

    /// Drops what the orchestrator holds in memory for a deleted chain. Its cards' runs end with their sessions (session.rs).
    fn forget(&self, chain_id: &str) {
        let mut sh = self.shared();
        sh.chains.remove(chain_id);
        sh.nudges.remove(chain_id);
        sh.spins.remove(chain_id);
    }
}
