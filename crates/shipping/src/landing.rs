//! The merge queue's rule (ADR-0114), pure: which landings of a queue go into the next batch, how big a batch grows, what
//! the forge's pipeline on a push means for what it carried, and what a revert or a re-queued copy of a landing is. The
//! shell (queue.rs) reads the queue, combines, checks, pushes through a `Lander` and records what this rule says; the
//! tests run the rule without a mock.
//!
//! The base only moves forward: a batch is combined on the commit the forge says the base stands at (`onto`), each
//! landing a commit on the one before, and the `Lander` moves the base from `onto` to the batch's top only as a
//! fast-forward. A base that moved meanwhile is combined again; nothing is ever forced.

use async_trait::async_trait;
use model::{Actor, Checks, ChecksState, Landing, LandingState, LandingVersion, QueueKey, Revert, RevertCause, Time};

/// The most landings one batch carries. A batch grows by one after each that lands and halves after one whose checks
/// fail, so a red batch is bisected down to the landing that broke it.
pub const MAX_BATCH: usize = 8;

/// The batch size after a batch of size k landed.
pub const fn grown(k: usize) -> usize {
    if k >= MAX_BATCH { MAX_BATCH } else { k.saturating_add(1) }
}

/// The batch size after a batch of size k failed its checks: half, never below one.
pub const fn halved(k: usize) -> usize {
    if k <= 1 { 1 } else { k / 2 }
}

/// How the move of a base went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pushed {
    /// The base stands at the top now.
    Landed,
    /// The base moved past onto meanwhile: the batch is combined again on where it stands.
    Moved,
    /// The forge refused the push, in its words: a protected base Remit's account may not push to.
    Refused(String),
}

/// The port the queue lands through: it moves a queue's base from onto to top, a fast-forward and nothing else. One
/// adapter is built, `DirectPush` (direct_push.rs); a forge that lands through its own merge API (`ForgeMerge`) is a seam
/// left unbuilt (ADR-0114 *Revisit when*). An error is a failure to ask (the network, the forge down): the batch waits.
#[async_trait]
pub trait Lander: Send + Sync {
    async fn land(&self, clone: &str, q: &QueueKey, onto: &str, top: &str) -> Result<Pushed, String>;
}

/// The order a queue lands its ready landings in: reverts first, the one taking back the newest landing first, as
/// taking a change back is what the queue exists to do fast and a later change may build on an earlier; then the rest in
/// the order they became ready. `queue` is every landing of the queue, so a revert's target is found there.
pub fn ordered(queue: &[Landing]) -> Vec<&Landing> {
    let landed_at = |id: &str| queue.iter().find(|l| l.id == id).and_then(|l| l.landed_at);
    let mut ready: Vec<&Landing> = queue.iter().filter(|l| l.state == LandingState::Ready).collect();
    ready.sort_by(|a, b| {
        let rank = |l: &Landing| u8::from(l.reverts.is_none());
        rank(a)
            .cmp(&rank(b))
            .then_with(|| match (a.reverted_one(), b.reverted_one()) {
                (Some(x), Some(y)) => landed_at(y).cmp(&landed_at(x)),
                _ => (a.ready_at, a.created_at).cmp(&(b.ready_at, b.created_at)),
            })
    });
    ready
}

/// The next batch of queue, at most k landings, in the order they land (`ordered`). Reverts go only with reverts; a stage
/// of a canary (a landing with a plan) goes alone, as the rule that lets it land reads the stage before it; a landing in
/// held (a stage the rollout rule holds now) waits, and the queue goes on past it. Empty when nothing is ready.
pub fn next_batch(queue: &[Landing], k: usize, held: &[String]) -> Vec<Landing> {
    let mut batch: Vec<Landing> = Vec::new();
    for l in ordered(queue).into_iter().filter(|l| !held.contains(&l.id)) {
        let Some(first) = batch.first() else {
            batch.push(l.clone());
            if l.version.plan.is_some() {
                break;
            }
            continue;
        };
        if batch.len() >= k.max(1) || l.version.plan.is_some() || l.reverts.is_none() != first.reverts.is_none() {
            break;
        }
        batch.push(l.clone());
    }
    batch
}

/// What the forge's pipeline on a push says of what it carried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Nothing settled yet: it is read again.
    Wait,
    /// The push stands: it passed, or the base before it was red already, so the red is not its doing.
    Stands,
    /// The push failed where the base before it passed: what it carried is reverted.
    Revert,
}

/// The rule on a push's pipeline (`push`) and the one on the base before it (`onto`). A push is reverted only when its
/// own checks failed and the base's passed: then the push broke it. None is a pipeline that has said nothing yet.
pub fn verdict_on(push: Option<&Checks>, onto: Option<&Checks>) -> Verdict {
    match (push.map(|c| c.state), onto.map(|c| c.state)) {
        (Some(ChecksState::Passed), _) | (Some(ChecksState::Failed), Some(ChecksState::Failed)) => Verdict::Stands,
        (Some(ChecksState::Failed), Some(ChecksState::Passed)) => Verdict::Revert,
        _ => Verdict::Wait,
    }
}

/// A revert of target, the queue's own landing `id`, ready at once, for cause, on the word of by, for why: it lands ahead
/// of every change (`ordered`) and needs no verdict, as taking a landed change back is the rail itself. Its chain is
/// target's, whose Chair is told. Target stays on the base until it lands (`taken_back`).
pub fn revert_of(target: &Landing, id: &str, cause: RevertCause, by: Actor, why: &str, now: Time) -> Landing {
    Landing {
        id: id.to_owned(),
        chain_id: target.chain_id.clone(),
        repo: target.repo.clone(),
        queue: target.queue.clone(),
        version: LandingVersion {
            number: target.version.number,
            title: format!("Revert \"{}\"", target.version.title),
            description: format!(
                "This reverts commit {}.\n\n{why}",
                target.commit.as_deref().unwrap_or_default()
            ),
            paths: target.version.paths.clone(),
            ..LandingVersion::default()
        },
        reverts: Some(Revert {
            landing: target.id.clone(),
            cause,
            by,
            why: why.to_owned(),
        }),
        state: LandingState::Ready,
        ready_at: Some(now),
        created_at: now,
        updated_at: now,
        ..Landing::default()
    }
}

/// target once revert, the landing that takes it back, landed: off the base, saying what took it back, who and why.
pub fn taken_back(mut target: Landing, revert: &Landing, now: Time) -> Landing {
    if let Some(r) = &revert.reverts {
        target.state = LandingState::Reverted {
            revert: revert.id.clone(),
            cause: r.cause,
            by: r.by.clone(),
            why: r.why.clone(),
        };
        target.reverted_at = Some(now);
        target.updated_at = now;
    }
    target
}

/// The revert, of queue (every landing of l's queue), that is taking l back or took it back; none when nothing does.
pub fn reverting<'a>(l: &Landing, queue: &'a [Landing]) -> Option<&'a Landing> {
    queue
        .iter()
        .find(|r| r.taking_back() == Some(l.id.as_str()) || (r.on_base() && r.reverted_one() == Some(l.id.as_str())))
}

/// Whether a revert of l may be queued, of queue (every landing of l's queue): l is on the base and nothing takes it back
/// yet; a revert is never reverted (the change it took back lands again as a new version of its chain).
pub fn revertible(l: &Landing, queue: &[Landing]) -> bool {
    l.on_base() && l.reverts.is_none() && reverting(l, queue).is_none()
}

/// A copy of l, `id`, ready again: what a push the pipeline broke carried along with others is queued once more, so the
/// bisecting batches find which of them broke it.
pub fn requeued(l: &Landing, id: &str, now: Time) -> Landing {
    Landing {
        id: id.to_owned(),
        state: LandingState::Ready,
        batch: None,
        onto: None,
        commit: None,
        push: None,
        checks: None,
        base_checks: None,
        release: None,
        reference: None,
        ready_at: Some(now),
        landed_at: None,
        reverted_at: None,
        created_at: now,
        updated_at: now,
        rev: 0,
        ..l.clone()
    }
}

/// The message of the commit landing l becomes: its title and description, then trailers that tie the commit to Remit's
/// record (its chain, the key result it serves, the verdicts that approved it, the landing) and credit the authors of
/// the work it squashes.
pub fn commit_message(l: &Landing, serves: &str, co_authors: &[String]) -> String {
    let mut m = l.version.title.trim().to_owned();
    if !l.version.description.trim().is_empty() {
        m.push_str("\n\n");
        m.push_str(l.version.description.trim());
    }
    m.push_str("\n\n");
    for a in co_authors {
        m.push_str(&format!("Co-authored-by: {a}\n"));
    }
    m.push_str(&format!("Remit-Chain: {}\n", l.chain_id));
    if !serves.is_empty() {
        m.push_str(&format!("Remit-Serves: {serves}\n"));
    }
    if !l.version.verdicts.is_empty() {
        m.push_str(&format!("Remit-Verdicts: {}\n", l.version.verdicts.join(", ")));
    }
    if let Some(target) = l.reverted_one() {
        m.push_str(&format!("Remit-Reverts: {target}\n"));
    }
    m.push_str(&format!("Remit-Landing: {}\n", l.id));
    m
}

#[cfg(test)]
mod tests;
