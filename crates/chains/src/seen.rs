//! What the owner has seen (ADR-0079): a mark per home (a chain's, the Assistant's) and one on the Assistant's feed, kept
//! by the server beside the records they annotate, so marking moves no chain's revision. The browser reports a real
//! view (`seen_mark`); the waits that began after a home's mark are unseen (waiting.rs), a chain that finished after its
//! mark is new on the board, and opening a chain shows what was decided in it while he was away.

use model::{Chain, ChainStatus, Decision, DecisionKind, Time, by_remit};
use wire::app::{Away, AwayLine};

/// How many of the decisions made while the owner was away a chain's view shows: the newest.
const AWAY_SHOWN: usize = 8;

/// What of decisions, a chain's oldest first, came after the owner's mark on it; none when he never viewed it or nothing
/// was decided since. Pure.
pub fn away_of(decisions: &[Decision], mark: Option<Time>) -> Option<Away> {
    let since = mark?;
    let after: Vec<&Decision> = decisions.iter().filter(|d| d.created_at > since).collect();
    if after.is_empty() {
        return None;
    }
    let more = after.len().saturating_sub(AWAY_SHOWN);
    let lines = after
        .iter()
        .skip(more)
        .map(|d| AwayLine {
            at: d.created_at,
            // a decision from before Remit recorded who made it
            who: d.by.clone().unwrap_or_else(|| by_remit("")),
            text: d.text.clone(),
        })
        .collect();
    Some(Away { since, lines, more })
}

/// The decisions that end a chain's run: its last of them is when it finished.
const ENDINGS: [DecisionKind; 3] = [DecisionKind::Finish, DecisionKind::Stall, DecisionKind::GoalFailed];

/// When chain c last finished (its newest finish, stall or goal_failed decision); none while it is not finished.
pub fn ended_at(r: &store::Reader<'_>, c: &Chain) -> store::Result<Option<Time>> {
    if *c.status() != ChainStatus::Done {
        return Ok(None);
    }
    let mut at = None;
    for kind in &ENDINGS {
        for d in r.decisions_of_kind(&c.id, kind)? {
            at = at.max(Some(d.created_at));
        }
    }
    Ok(at)
}

/// Whether a chain that last finished at ended (none: it has not) did so after the owner's mark on it. One he never
/// viewed is new. Pure.
pub fn finished_unseen(ended: Option<Time>, mark: Option<Time>) -> bool {
    ended.is_some_and(|ended| mark.is_none_or(|at| ended > at))
}
