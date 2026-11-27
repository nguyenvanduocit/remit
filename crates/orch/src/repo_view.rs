//! A repository's queue and Releases views (ADR-0114, ADR-0116), laid out from its landings and releases as stored:
//! each queue in the order it takes its landings, the newest releases with what they carried. Read for `repo_get`, and
//! computed again and pushed whole whenever a landing or a release of the repository is written (views.rs).

use std::collections::{BTreeMap, BTreeSet};

use model::{Landing, LandingState, Release, ReleasePause, Time};
use wire::app::{LandingQueue, QUEUE_ENDED_SHOWN_HOURS, RELEASES_SHOWN, ReleaseRead, RepoView};

use crate::{Orchestrator, Result};

/// Where a landing stands in its queue's view: the batch, ready, or ended at a time.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Place {
    /// Combined or checked: the batch the queue works on.
    Batch,
    Ready,
    Ended,
}

/// Where l goes in its queue's view and when it entered or left the queue; none for a version still judged, or contending
/// for its round's pick.
fn place(l: &Landing) -> Option<(Place, Time)> {
    let entered = l.ready_at.unwrap_or(l.created_at);
    Some(match &l.state {
        LandingState::Judging | LandingState::Contending => return None,
        LandingState::Combining | LandingState::Checking => (Place::Batch, entered),
        LandingState::Ready => (Place::Ready, entered),
        LandingState::Landed | LandingState::Returned { .. } | LandingState::Reverted { .. } => {
            (Place::Ended, l.reverted_at.or(l.landed_at).unwrap_or(entered))
        }
    })
}

/// The queue of each base of landings, by base: the batch and what is ready in the order the queue takes them (the first
/// ready first), then what ended at or after since, the latest first. Pure.
fn queues(landings: Vec<Landing>, since: Time) -> Vec<LandingQueue> {
    let mut by_base: BTreeMap<String, Vec<(Place, Time, Landing)>> = BTreeMap::new();
    for l in landings {
        let Some((at, when)) = place(&l).filter(|_| l.queue.is_some()) else {
            continue; // a version of a folder worked in place waits in no queue
        };
        if at == Place::Ended && when < since {
            continue;
        }
        by_base.entry(l.base().to_owned()).or_default().push((at, when, l));
    }
    by_base
        .into_iter()
        .map(|(base, mut ls)| {
            ls.sort_by(|(a, at_a, la), (b, at_b, lb)| {
                let when = if *a == Place::Ended {
                    at_b.cmp(at_a)
                } else {
                    at_a.cmp(at_b)
                };
                a.cmp(b).then(when).then_with(|| la.id.cmp(&lb.id))
            });
            LandingQueue {
                base,
                landings: ls.into_iter().map(|(_, _, l)| l).collect(),
            }
        })
        .collect()
}

/// Repository repo's view at now, from its landings (any order), its newest releases with their landings and its last
/// release pause. Pure.
pub(crate) fn repo_view(
    repo: String,
    landings: Vec<Landing>,
    releases: Vec<ReleaseRead>,
    pause: Option<ReleasePause>,
    now: Time,
) -> RepoView {
    let since = now
        .0
        .checked_sub(time::Duration::hours(QUEUE_ENDED_SHOWN_HOURS))
        .map_or_else(Time::default, Time);
    RepoView {
        repo,
        queues: queues(landings, since),
        releases,
        pause,
    }
}

impl Orchestrator {
    /// Repository repo's queue and releases, as stored now.
    pub async fn repo_view(&self, repo: String) -> Result<RepoView> {
        let now = self.now();
        Ok(self.store.read(move |r| read_repo_view(r, repo, now)).await?)
    }

    /// The view of every repository a landing names whose queues or releases hold anything to show, by repository.
    pub async fn repo_views(&self) -> Result<Vec<RepoView>> {
        let now = self.now();
        Ok(self
            .store
            .read(move |r| {
                let repos: BTreeSet<String> = r.landings()?.into_iter().map(|l| l.repo).collect();
                repos
                    .into_iter()
                    .map(|repo| read_repo_view(r, repo, now))
                    .filter(|v| {
                        v.as_ref()
                            .map_or(true, |v| !v.queues.is_empty() || !v.releases.is_empty())
                    })
                    .collect::<store::Result<Vec<_>>>()
            })
            .await?)
    }
}

/// Repository repo's view as r holds it at now.
fn read_repo_view(r: &store::Reader<'_>, repo: String, now: Time) -> store::Result<RepoView> {
    let landings = r.repo_landings(&repo)?;
    let releases = r
        .repo_releases(&repo)?
        .into_iter()
        .take(RELEASES_SHOWN)
        .map(|release: Release| {
            let landings = r.release_landings(&release.id)?;
            Ok(ReleaseRead { release, landings })
        })
        .collect::<store::Result<Vec<_>>>()?;
    let pause = r.release_pause(&repo)?;
    Ok(repo_view(repo, landings, releases, pause, now))
}
