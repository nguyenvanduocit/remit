//! The pure core of a replay (ADR-0069): which recorded goals a change's candidate is run against, how one goal fared
//! under the goal judge, and what a replay's goals come to. It stores, starts and reads nothing; the orchestrator runs the
//! chains and keeps the replay on its change item.

use model::{
    Approval, Card, Chain, ChainStatus, ChangeKind, FindingBlocker, FindingStatus, ReplayBrief, ReplayOutcome,
    ReplayScore, ReplayVerdict, VerdictRole,
};

/// How many recorded goals a replay runs: the newest that used the candidate. One goal judge run each, so a score moves
/// by one goal's noise; widening waits for that noise to be measured (ADR-0069, Revisit when).
pub const SIZE: usize = 20;

/// The record a change item would write, as a replay selects by it: an agent template or a skill, by id or name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub kind: ChangeKind,
    pub name: String,
}

/// A chain as a replay reads it: the chain, the cards that worked in it and every verdict given on it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Record {
    pub chain: Chain,
    pub cards: Vec<Card>,
    pub approvals: Vec<Approval>,
}

/// The goals a replay of `c` runs, newest first, at most `n`: chains that finished with a verdict of the goal judge, that
/// a card holding `c` worked in (a card hired from the agent template, or holding the skill), that are no replay
/// themselves, and that held no capability in `writers`, so running their goal again writes nothing a person relies on.
/// Each carries how the goal judge ruled on it then.
pub fn select(c: &Candidate, records: &[Record], writers: &[String], n: usize) -> Vec<ReplayBrief> {
    let mut picked: Vec<&Record> = records
        .iter()
        .filter(|r| {
            *r.chain.status() == ChainStatus::Done
                && r.chain.replay.is_empty()
                && outcome_of(&r.approvals) != ReplayOutcome::Unjudged
        })
        .filter(|r| !r.chain.capabilities.iter().any(|id| writers.contains(id)))
        .filter(|r| r.cards.iter().any(|card| holds(card, c)))
        .collect();
    picked.sort_by(|a, b| {
        b.chain
            .created_at
            .cmp(&a.chain.created_at)
            .then_with(|| a.chain.id.cmp(&b.chain.id))
    });
    picked
        .into_iter()
        .take(n)
        .map(|r| ReplayBrief {
            chain: r.chain.id.clone(),
            title: r.chain.title.clone(),
            recorded: outcome_of(&r.approvals),
            ..ReplayBrief::default()
        })
        .collect()
}

/// Whether `card` worked with `c`: hired from agent template `c`, or holding skill `c`.
fn holds(card: &Card, c: &Candidate) -> bool {
    match c.kind {
        ChangeKind::Agent => card.agent == c.name,
        ChangeKind::Skill => card.loadout.as_ref().is_some_and(|l| l.skills.contains(&c.name)),
        _ => false,
    }
}

/// How a chain fared under the goal judge, from its verdicts: its latest goal check is met when no finding is not_met and
/// none is left unproven but on a merge or a deploy, which a replay never makes (a replay publishes nothing, and a
/// recorded chain's merge was the owner's). A chain the goal judge never ruled on is unjudged.
pub fn outcome_of(approvals: &[Approval]) -> ReplayOutcome {
    let Some(last) = approvals.iter().rev().find(|a| a.role == VerdictRole::Judge) else {
        return ReplayOutcome::Unjudged;
    };
    let fails = last.findings.iter().any(|f| {
        f.status == FindingStatus::NotMet
            || (f.status == FindingStatus::Unverified
                && !matches!(f.blocker, Some(FindingBlocker::Write | FindingBlocker::Deploy)))
    });
    if fails {
        ReplayOutcome::NotMet
    } else {
        ReplayOutcome::Met
    }
}

/// How the replay chain `ch` fares: pending until it finishes, then its goal judge's outcome.
pub fn outcome_of_replay(ch: &Chain, approvals: &[Approval]) -> ReplayOutcome {
    if *ch.status() != ChainStatus::Done {
        return ReplayOutcome::Pending;
    }
    outcome_of(approvals)
}

/// The index of the brief to queue next, or none: one goal is replayed at a time, so a replay holds one slot of the
/// owner's work.
pub fn next(briefs: &[ReplayBrief]) -> Option<usize> {
    if briefs.iter().any(in_flight) {
        return None;
    }
    briefs
        .iter()
        .position(|b| b.replay.is_empty() && b.outcome == ReplayOutcome::Pending)
}

fn in_flight(b: &ReplayBrief) -> bool {
    !b.replay.is_empty() && b.outcome == ReplayOutcome::Pending
}

/// What `briefs` come to. Only a goal judged both times is compared; the candidate regressed when it met fewer of them
/// than were met when they were recorded, and every goal that went from met to not met is named, as is every one that went
/// the other way. A replay with a goal still to run is running, and `stopped` says its item was answered or replaced
/// before it ended.
pub fn score(briefs: &[ReplayBrief], stopped: bool) -> ReplayScore {
    let mut s = ReplayScore::default();
    let mut pending = false;
    for b in briefs {
        if b.outcome == ReplayOutcome::Pending {
            pending = true;
            continue;
        }
        if b.outcome == ReplayOutcome::Unjudged || b.recorded == ReplayOutcome::Unjudged {
            continue;
        }
        s.judged = s.judged.saturating_add(1);
        let (recorded, now) = (b.recorded == ReplayOutcome::Met, b.outcome == ReplayOutcome::Met);
        if recorded {
            s.recorded_met = s.recorded_met.saturating_add(1);
        }
        if now {
            s.replay_met = s.replay_met.saturating_add(1);
        }
        match (recorded, now) {
            (true, false) => s.regressed.push(b.chain.clone()),
            (false, true) => s.improved.push(b.chain.clone()),
            _ => {}
        }
    }
    s.verdict = if pending && stopped {
        ReplayVerdict::Stopped
    } else if pending {
        ReplayVerdict::Running
    } else if s.judged == 0 {
        ReplayVerdict::Inconclusive
    } else if s.replay_met < s.recorded_met {
        ReplayVerdict::Regressed
    } else {
        ReplayVerdict::Held
    };
    s
}
