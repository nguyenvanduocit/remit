//! The report: what happened in each period of a span and in all time, by role, model and kind of chain.

use std::collections::{BTreeMap, HashMap, HashSet};

use model::{
    Approval, Card, CardStatus, Chain, ChainOrigin, Decision, DecisionKind, Landing, LandingState, OutcomeChainKind,
    OutcomeModel, OutcomePeriod, OutcomeReport, OutcomeRole, ReturnReason, RevertCause, SeededFault, Time, ToolTally,
    Verdict, VerdictRole,
};

use crate::kpi::kpi_groups;
use crate::usage::usages;
use crate::{Span, float};

/// The record the report is computed from.
#[derive(Debug, Clone, Copy, Default)]
pub struct Input<'a> {
    pub now: Time,
    pub chains: &'a [Chain],
    pub cards: &'a [Card],
    pub approvals: &'a [Approval],
    pub decisions: &'a [Decision],
    /// Every landing of every chain (ADR-0114).
    pub landings: &'a [Landing],
    /// Every tool call of every agent, counted per card and tool.
    pub tool_calls: &'a [ToolTally],
    /// The faults planted in trial chains (ADR-0102): the verdicts on those chains count only as catches.
    pub seeded: &'a [SeededFault],
}

fn bump(n: &mut i64) {
    *n = n.saturating_add(1);
}

/// The roles whose catches of a planted fault are counted.
const CATCHING: [VerdictRole; 3] = [VerdictRole::Reviewer, VerdictRole::Qc, VerdictRole::Judge];

/// Counts, for each planted fault, the first verdict each catching role gave on its chain after it was planted: caught
/// when that verdict did not approve. `trial` is the verdicts on trial chains, oldest first.
fn count_catches(seeded: &[SeededFault], trial: &[&Approval], counter: &mut Counter) {
    for f in seeded {
        for role in CATCHING {
            let Some(a) = trial
                .iter()
                .find(|a| a.chain_id == f.chain_id && a.role == role && a.at >= f.planted_at)
            else {
                continue;
            };
            let caught = a.verdict != Verdict::Approve;
            counter.count(a.at, |w| {
                let (ruled, got) = match role {
                    VerdictRole::Reviewer => (&mut w.reviewer_faults, &mut w.reviewer_caught),
                    VerdictRole::Qc => (&mut w.qc_faults, &mut w.qc_caught),
                    _ => (&mut w.judge_faults, &mut w.judge_caught),
                };
                bump(ruled);
                if caught {
                    bump(got);
                }
            });
        }
    }
}

const HOUR_NANOS: i64 = 3_600_000_000_000;

/// The hours from `from` to `to`; none when `to` is before it.
fn hours_between(from: Time, to: Time) -> Option<f64> {
    if to < from {
        return None;
    }
    // The span in nanoseconds as an i64, the longest one when it is past it.
    let span =
        to.0.unix_timestamp_nanos()
            .saturating_sub(from.0.unix_timestamp_nanos());
    let span = i64::try_from(span).unwrap_or(i64::MAX);
    // Whole hours and the rest apart, so a long span keeps its precision.
    let (hours, rest) = (span.checked_div(HOUR_NANOS)?, span.checked_rem(HOUR_NANOS)?);
    Some(float(hours) + float(rest) / float(HOUR_NANOS))
}

/// Counts each landing of a chain's change where it fell: when it landed (with its batch, once, and its wait in the
/// queue), when it went back to its Chair for what the queue found, and when it was taken back. A revert is not counted:
/// it is what the landing it takes back counts as.
fn count_landings(landings: &[&Landing], counter: &mut Counter) {
    let mut batches: HashSet<&str> = HashSet::new();
    for l in landings.iter().filter(|l| l.reverts.is_none()) {
        if let Some(at) = l.landed_at {
            let new_batch = l.batch.as_deref().is_some_and(|b| batches.insert(b));
            let waited = l.ready_at.and_then(|ready| hours_between(ready, at));
            counter.count(at, |w| {
                bump(&mut w.landed);
                if new_batch {
                    bump(&mut w.batches);
                }
                if let Some(h) = waited {
                    w.queue_hours += h;
                }
            });
        }
        if let LandingState::Returned { reason, .. } = &l.state {
            let (counts, conflict, checks) = match reason {
                ReturnReason::Conflict => (true, true, false),
                ReturnReason::Checks => (true, false, true),
                ReturnReason::Empty | ReturnReason::Refused => (true, false, false),
                // a newer version, a deleted chain, or another attempt of its round picked to land says nothing of what
                // the queue found
                ReturnReason::Replaced | ReturnReason::Withdrawn | ReturnReason::NotPicked => (false, false, false),
            };
            if counts {
                counter.count(l.updated_at, |w| {
                    bump(&mut w.returned);
                    if conflict {
                        bump(&mut w.returned_conflict);
                    }
                    if checks {
                        bump(&mut w.returned_checks);
                    }
                });
            }
        }
        if let (LandingState::Reverted { cause, .. }, Some(at)) = (&l.state, l.reverted_at) {
            let overturned = *cause == RevertCause::Overturn;
            counter.count(at, |w| {
                bump(&mut w.reverted);
                if overturned {
                    bump(&mut w.overturned);
                }
            });
        }
    }
}

/// The Chair's first word on each report, by the card that wrote it.
fn first_judgements<'a>(ds: &[&'a Decision]) -> HashMap<&'a str, &'a Decision> {
    let mut sorted: Vec<&Decision> = ds.to_vec();
    sorted.sort_by_key(|d| d.created_at);
    let mut first = HashMap::new();
    for d in sorted {
        if matches!(d.kind, DecisionKind::Accept | DecisionKind::Rework) && !d.card_id.is_empty() {
            first.entry(d.card_id.as_str()).or_insert(d);
        }
    }
    first
}

/// Adds each event to the total and to the period it fell in, if it is in the span.
struct Counter {
    now: i128,
    window: i128,
    all: OutcomePeriod,
    periods: Vec<OutcomePeriod>,
}

impl Counter {
    /// Adds one event at `at` through `add`. An event with no time is in the total only.
    fn count(&mut self, at: impl Into<Option<Time>>, add: impl Fn(&mut OutcomePeriod)) {
        add(&mut self.all);
        let Some(at) = at.into() else {
            return;
        };
        let age = self.now.saturating_sub(at.0.unix_timestamp_nanos());
        if age < 0 {
            return;
        }
        let back = age.checked_div(self.window).and_then(|b| usize::try_from(b).ok());
        let index = back.and_then(|b| self.periods.len().checked_sub(1)?.checked_sub(b));
        if let Some(p) = index.and_then(|i| self.periods.get_mut(i)) {
            add(p);
        }
    }
}

/// What the approvals say of one chain: the goal judge's first and latest verdict, and whether a reviewer or QC judged
/// it and asked for changes.
#[derive(Clone, Copy, Default)]
struct ChainFacts<'a> {
    first: Option<&'a Approval>,
    last: Option<&'a Approval>,
    gated: bool,
    rework: bool,
}

impl ChainFacts<'_> {
    /// Counts the chain's goal check: judged on its first verdict (did it pass at once) and on its latest (did it confirm
    /// the result).
    fn count_goal(&self, p: &mut OutcomeChainKind, counter: &mut Counter) {
        if let Some(first) = self.first {
            bump(&mut p.goal_checked);
            let passed = first.verdict == Verdict::Approve;
            if passed {
                bump(&mut p.goal_first_pass);
            }
            counter.count(first.at, |w| {
                bump(&mut w.goal_checked);
                if passed {
                    bump(&mut w.goal_first_pass);
                }
            });
        }
        if let Some(last) = self.last {
            let confirmed = last.confirms();
            counter.count(last.at, |w| {
                bump(&mut w.goal_ruled);
                if confirmed {
                    bump(&mut w.goal_confirmed);
                }
            });
        }
    }
}

/// The report for what is recorded, as of `input.now`, over span `span`.
#[must_use]
pub fn compute(input: &Input<'_>, span: &Span) -> OutcomeReport {
    let now = input.now;
    let n = span.periods();
    let periods = (0..n)
        .map(|i| {
            // A few hundred days at most: neither product overflows.
            let back = i64::try_from(n.saturating_sub(i)).unwrap_or(i64::MAX);
            let days = back.saturating_mul(span.window_days());
            let start = now
                .0
                .saturating_sub(time::Duration::seconds(days.saturating_mul(86_400)));
            OutcomePeriod {
                start: Time(start),
                ..OutcomePeriod::default()
            }
        })
        .collect();
    let mut counter = Counter {
        now: now.0.unix_timestamp_nanos(),
        window: span.window_nanos(),
        all: OutcomePeriod::default(),
        periods,
    };

    // The chains every count but the catches leaves out: a client key's, whose work is not the owner's, and the trial
    // chains, whose work carries a fault on purpose, so their verdicts measure the judges alone.
    let trials: HashSet<&str> = input.seeded.iter().map(|f| f.chain_id.as_str()).collect();
    let left_out: HashSet<&str> = input
        .chains
        .iter()
        .filter(|c| c.origin == ChainOrigin::Client)
        .map(|c| c.id.as_str())
        .chain(trials.iter().copied())
        .collect();
    let counted = |chain_id: &str| !left_out.contains(chain_id);
    let chains: Vec<&Chain> = input.chains.iter().filter(|c| counted(&c.id)).collect();
    let cards: Vec<&Card> = input.cards.iter().filter(|c| counted(&c.home)).collect();
    let decisions: Vec<&Decision> = input.decisions.iter().filter(|d| counted(&d.chain_id)).collect();

    let mut approvals: Vec<&Approval> = input.approvals.iter().collect();
    approvals.sort_by_key(|a| a.at);
    let (trial, approvals): (Vec<&Approval>, Vec<&Approval>) = approvals
        .into_iter()
        .filter(|a| trials.contains(a.chain_id.as_str()) || counted(&a.chain_id))
        .partition(|a| trials.contains(a.chain_id.as_str()));
    count_catches(input.seeded, &trial, &mut counter);
    let mut by_chain: HashMap<&str, ChainFacts> = HashMap::new();
    for &a in &approvals {
        if a.role == VerdictRole::Judge {
            let f = by_chain.entry(a.chain_id.as_str()).or_default();
            f.first = f.first.or(Some(a));
            f.last = Some(a);
        } else if matches!(a.role, VerdictRole::Reviewer | VerdictRole::Qc) {
            let f = by_chain.entry(a.chain_id.as_str()).or_default();
            let rework = a.verdict != Verdict::Approve;
            f.gated = true;
            f.rework = f.rework || rework;
            counter.count(a.at, |w| {
                bump(&mut w.verdicts);
                if rework {
                    bump(&mut w.changes_requested);
                }
            });
        }
    }

    let landings: Vec<&Landing> = input.landings.iter().filter(|l| counted(&l.chain_id)).collect();
    count_landings(&landings, &mut counter);

    let mut kinds: BTreeMap<&str, OutcomeChainKind> = BTreeMap::new();
    for &c in &chains {
        counter.count(c.created_at, |w| bump(&mut w.chains));
        let p = kinds.entry(c.kind.as_str()).or_insert_with(|| OutcomeChainKind {
            kind: c.kind,
            ..OutcomeChainKind::default()
        });
        bump(&mut p.chains);
        let f = by_chain.get(c.id.as_str()).copied().unwrap_or_default();
        f.count_goal(p, &mut counter);
        if f.gated {
            bump(&mut p.gated);
        }
        if f.rework {
            bump(&mut p.reworked);
        }
        let own = landings.iter().filter(|l| l.chain_id == c.id && l.reverts.is_none());
        let (mut landed, mut reverted, mut on_way) = (false, false, false);
        for l in own {
            landed = landed || l.on_base();
            reverted = reverted || matches!(l.state, LandingState::Reverted { .. });
            on_way = on_way || l.state.open();
        }
        for (n, yes) in [
            (&mut p.landed, landed),
            (&mut p.reverted, reverted),
            (&mut p.landing, on_way),
        ] {
            if yes {
                bump(n);
            }
        }
    }

    let first = first_judgements(&decisions);
    for d in first.values() {
        let accepted = d.kind == DecisionKind::Accept;
        counter.count(d.created_at, |w| {
            bump(if accepted { &mut w.accepted } else { &mut w.sent_back })
        });
    }
    for d in &decisions {
        match d.kind {
            DecisionKind::GoalFailed => counter.count(d.created_at, |w| bump(&mut w.goal_not_run)),
            DecisionKind::Stall => counter.count(d.created_at, |w| bump(&mut w.stalled)),
            // Only the two decisions that count against a period are met here.
            _ => {}
        }
    }

    let mut roles: HashMap<String, OutcomeRole> = HashMap::new();
    let mut models: HashMap<(&str, &str), OutcomeModel> = HashMap::new();
    let mut role_of: HashMap<&str, String> = HashMap::new();
    for &c in &cards {
        let role = c.agent.trim().to_lowercase();
        let mdl = if c.model.is_empty() {
            "unknown"
        } else {
            c.model.as_str()
        };
        role_of.insert(c.id.as_str(), role.clone());
        let failed = c.status() == &CardStatus::Failed;
        let ro = roles.entry(role.clone()).or_insert_with(|| OutcomeRole {
            role,
            ..OutcomeRole::default()
        });
        bump(&mut ro.agents);
        ro.turns = ro.turns.saturating_add(c.turns);
        let mo = models.entry((mdl, c.effort.as_str())).or_insert_with(|| OutcomeModel {
            model: mdl.to_owned(),
            effort: c.effort.clone(),
            ..OutcomeModel::default()
        });
        bump(&mut mo.agents);
        mo.turns = mo.turns.saturating_add(c.turns);
        if failed {
            bump(&mut ro.failed);
            bump(&mut mo.failed);
            counter.count(c.updated_at, |w| bump(&mut w.failed_agents));
        }
    }
    for (id, d) in &first {
        if let Some(ro) = role_of.get(id).and_then(|role| roles.get_mut(role)) {
            bump(if d.kind == DecisionKind::Accept {
                &mut ro.accepted
            } else {
                &mut ro.sent_back
            });
        }
    }
    // A verdict counts for the agent of the card that gave it, as a report does.
    for a in &approvals {
        if let Some(ro) = role_of.get(a.card_id.as_str()).and_then(|role| roles.get_mut(role)) {
            bump(if a.verdict == Verdict::Approve {
                &mut ro.approved
            } else {
                &mut ro.changes_requested
            });
        }
    }
    let mut roles: Vec<OutcomeRole> = roles.into_values().collect();
    roles.sort_by(|a, b| b.agents.cmp(&a.agents).then_with(|| a.role.cmp(&b.role)));
    let mut models: Vec<OutcomeModel> = models.into_values().collect();
    models.sort_by(|a, b| {
        b.agents
            .cmp(&a.agents)
            .then_with(|| a.model.cmp(&b.model))
            .then_with(|| a.effort.cmp(&b.effort))
    });

    let Counter { all, periods, .. } = counter;
    let used = usages(input.tool_calls, input.cards, input.chains);
    OutcomeReport {
        at: now,
        span: span.shown(),
        groups: kpi_groups(&periods, &all),
        periods,
        all,
        roles,
        models,
        kinds: kinds.into_values().collect(),
        tools: used.tools,
        mcp: used.mcp,
        skills: used.skills,
    }
}
