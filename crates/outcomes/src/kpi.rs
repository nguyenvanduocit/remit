//! The KPIs: each one's definition, its figures over a report's periods, and what their move says.

use model::{
    OutcomeBetter, OutcomeChange, OutcomeGroup, OutcomeGroupKey, OutcomeKind, OutcomeKpi, OutcomePeriod, OutcomePoint,
    OutcomeReads, OutcomeReport,
};

use crate::float;

/// The fewest events a share or an average must be over, in both windows, for its move to be read.
pub const MIN_EVENTS: i64 = 5;

fn share(n: i64, of: i64) -> OutcomePoint {
    if of == 0 {
        return OutcomePoint::default();
    }
    OutcomePoint {
        value: float(n) / float(of),
        n,
        of,
        has: true,
    }
}

fn count(n: i64) -> OutcomePoint {
    OutcomePoint {
        value: float(n),
        n,
        of: 0,
        has: true,
    }
}

/// `total` (hours, or a count) over `of` events; nothing when there are none.
fn average(total: f64, of: i64) -> OutcomePoint {
    if of == 0 {
        return OutcomePoint::default();
    }
    OutcomePoint {
        value: total / float(of),
        n: 0,
        of,
        has: true,
    }
}

struct Spec {
    key: &'static str,
    label: &'static str,
    hint: &'static str,
    kind: OutcomeKind,
    better: OutcomeBetter,
    /// What an average is over; "" for a share or a count.
    over: &'static str,
    of: fn(&OutcomePeriod) -> OutcomePoint,
}

struct GroupSpec {
    key: OutcomeGroupKey,
    label: &'static str,
    specs: &'static [Spec],
}

/// Every KPI: its definition lives here and nowhere else.
const GROUPS: &[GroupSpec] = &[
    // The loss the agents are steered to lower, read first, with the constraint it is read against beside it: reverts per
    // result fall when results rise, and fall falsely when the judge confirms nothing, so the two are one reading
    // (ADR-0068, ADR-0114).
    GroupSpec {
        key: OutcomeGroupKey::Headline,
        label: "Loss",
        specs: &[
            Spec {
                key: "reverts-per-result",
                label: "Landings taken back per confirmed result",
                hint: "The landings taken back in the period (by the forge's pipeline, by a guardrail, by a revert they conflicted with, or by the owner), over the results the goal judge confirmed in the same period. A revert is a defect that reached the base; an overturn is one that reached the owner. Read it with the results confirmed beside it: fewer reverts because nothing landed is no gain.",
                kind: OutcomeKind::Mean,
                better: OutcomeBetter::Down,
                over: "confirmed result",
                of: |w| average(float(w.reverted), w.goal_confirmed),
            },
            Spec {
                key: "goal-confirmed",
                label: "Results confirmed",
                hint: "Of the chains the goal judge has ruled on, those whose latest check approved with every key result met: what the goal promised the customer, shown to hold. An approval that leaves a key result unverified is not counted.",
                kind: OutcomeKind::Rate,
                better: OutcomeBetter::Up,
                over: "",
                of: |w| share(w.goal_confirmed, w.goal_ruled),
            },
        ],
    },
    GroupSpec {
        key: OutcomeGroupKey::Quality,
        label: "Quality",
        specs: &[
            Spec {
                key: "accepted-first-time",
                label: "Reports accepted first time",
                hint: "Of the reports agents handed their Chair, those the Chair accepted without sending back. A report counts once, on the Chair's first word on it.",
                kind: OutcomeKind::Rate,
                better: OutcomeBetter::Up,
                over: "",
                of: |w| share(w.accepted, w.accepted.saturating_add(w.sent_back)),
            },
            Spec {
                key: "goal-first-pass",
                label: "Goal check passed first time",
                hint: "Of the chains the goal judge checked, those it passed the first time the Chair asked to finish.",
                kind: OutcomeKind::Rate,
                better: OutcomeBetter::Up,
                over: "",
                of: |w| share(w.goal_first_pass, w.goal_checked),
            },
            Spec {
                key: "changes-requested",
                label: "Verdicts asking for changes",
                hint: "Of the verdicts reviewers and QC gave, those asking for changes. High can mean careful judges or weak work; read it with the first two.",
                kind: OutcomeKind::Rate,
                better: OutcomeBetter::Neither,
                over: "",
                of: |w| share(w.changes_requested, w.verdicts),
            },
            Spec {
                key: "faults-caught-reviewer",
                label: "Faults reviewers caught",
                hint: "Of the faults planted on purpose in trial chains that a reviewer ruled on, those its first verdict after the planting asked changes for: the reviewers' recall on a fault known to be there. A trial chain's verdicts count here and in the two below, never in the figures above.",
                kind: OutcomeKind::Rate,
                better: OutcomeBetter::Up,
                over: "",
                of: |w| share(w.reviewer_caught, w.reviewer_faults),
            },
            Spec {
                key: "faults-caught-qc",
                label: "Faults QC caught",
                hint: "Of the faults planted on purpose in trial chains that a QC ruled on, those its first verdict after the planting asked changes for.",
                kind: OutcomeKind::Rate,
                better: OutcomeBetter::Up,
                over: "",
                of: |w| share(w.qc_caught, w.qc_faults),
            },
            Spec {
                key: "faults-caught-judge",
                label: "Faults goal judge caught",
                hint: "Of the faults planted on purpose in trial chains that the goal judge ruled on, those its first check after the planting did not pass: the last judge before a change ships, on a fault known to be there.",
                kind: OutcomeKind::Rate,
                better: OutcomeBetter::Up,
                over: "",
                of: |w| share(w.judge_caught, w.judge_faults),
            },
        ],
    },
    GroupSpec {
        key: OutcomeGroupKey::Landing,
        label: "Landing",
        specs: &[
            Spec {
                key: "queue-wait",
                label: "Wait in the queue",
                hint: "How long the landings that reached their base in the period waited from ready (every gate approved) to landed, on average: the time the queue takes to combine, check and push them.",
                kind: OutcomeKind::Hours,
                better: OutcomeBetter::Down,
                over: "landing",
                of: |w| average(w.queue_hours, w.landed),
            },
            Spec {
                key: "returned",
                label: "Versions sent back by the queue",
                hint: "Of the versions the queue landed or sent back to their Chair in the period, those sent back: for a conflict with the base, a check that failed on the combined tree, nothing left to land, a push the forge refused, or a pipeline that failed on it alone.",
                kind: OutcomeKind::Rate,
                better: OutcomeBetter::Down,
                over: "",
                of: |w| share(w.returned, w.returned.saturating_add(w.landed)),
            },
            Spec {
                key: "returned-conflict",
                label: "Sent back for a conflict",
                hint: "Of the versions the queue landed or sent back in the period, those sent back because they conflict with what landed before them: chains changing the same lines.",
                kind: OutcomeKind::Rate,
                better: OutcomeBetter::Down,
                over: "",
                of: |w| share(w.returned_conflict, w.returned.saturating_add(w.landed)),
            },
            Spec {
                key: "returned-checks",
                label: "Sent back by the combined checks",
                hint: "Of the versions the queue landed or sent back in the period, those whose checks failed once combined with the base: two changes that pass alone and break together.",
                kind: OutcomeKind::Rate,
                better: OutcomeBetter::Down,
                over: "",
                of: |w| share(w.returned_checks, w.returned.saturating_add(w.landed)),
            },
            Spec {
                key: "batch-size",
                label: "Landings per batch",
                hint: "How many landings each push carried, on average. The queue grows a batch by one after one that passes and halves it after one that fails.",
                kind: OutcomeKind::Mean,
                better: OutcomeBetter::Neither,
                over: "batch",
                of: |w| average(float(w.landed), w.batches),
            },
        ],
    },
    GroupSpec {
        key: OutcomeGroupKey::Reliability,
        label: "Reliability",
        specs: &[
            Spec {
                key: "failed-agents",
                label: "Agents that failed",
                hint: "Agents whose session ended in a failure (the process died or a turn errored and was never retried).",
                kind: OutcomeKind::Count,
                better: OutcomeBetter::Down,
                over: "",
                of: |w| count(w.failed_agents),
            },
            Spec {
                key: "goal-not-run",
                label: "Goal checks that could not run",
                hint: "Chains that finished without their goal check because the judge failed, was stopped or ran out of time.",
                kind: OutcomeKind::Count,
                better: OutcomeBetter::Down,
                over: "",
                of: |w| count(w.goal_not_run),
            },
            Spec {
                key: "stalled",
                label: "Chains finished as stalled",
                hint: "Chains Remit finished because nothing moved after repeated reminders.",
                kind: OutcomeKind::Count,
                better: OutcomeBetter::Down,
                over: "",
                of: |w| count(w.stalled),
            },
        ],
    },
];

/// Evaluates every KPI over the periods and the total. A span always has at least two periods; were there fewer, the
/// missing ones would read as no figure.
pub(crate) fn kpi_groups(periods: &[OutcomePeriod], all: &OutcomePeriod) -> Vec<OutcomeGroup> {
    GROUPS
        .iter()
        .map(|g| OutcomeGroup {
            key: g.key,
            label: g.label.to_owned(),
            kpis: g
                .specs
                .iter()
                .map(|s| {
                    let series: Vec<OutcomePoint> = periods.iter().map(s.of).collect();
                    let back = |n: usize| {
                        series
                            .len()
                            .checked_sub(n)
                            .and_then(|i| series.get(i))
                            .copied()
                            .unwrap_or_default()
                    };
                    let (now, before) = (back(1), back(2));
                    OutcomeKpi {
                        key: s.key.to_owned(),
                        label: s.label.to_owned(),
                        hint: s.hint.to_owned(),
                        kind: s.kind,
                        better: s.better,
                        over: s.over.to_owned(),
                        all: (s.of)(all),
                        change: change_of(now, before, s.better),
                        series,
                        now,
                        before,
                    }
                })
                .collect(),
        })
        .collect()
}

/// Reads the move from `before` to `now`.
fn change_of(now: OutcomePoint, before: OutcomePoint, better: OutcomeBetter) -> OutcomeChange {
    if !now.has || !before.has {
        return OutcomeChange {
            delta: 0.0,
            reads: OutcomeReads::None,
        };
    }
    let delta = now.value - before.value;
    let few = |p: OutcomePoint| p.of > 0 && p.of < MIN_EVENTS;
    let reads = if few(now) || few(before) {
        OutcomeReads::Few
    } else if delta == 0.0 {
        OutcomeReads::Same
    } else if better == OutcomeBetter::Neither {
        OutcomeReads::Moved
    } else if (delta > 0.0) == (better == OutcomeBetter::Up) {
        OutcomeReads::Better
    } else {
        OutcomeReads::Worse
    };
    OutcomeChange { delta, reads }
}

/// Figure p of a KPI of kind kind, its average over over, in words.
fn point_words(p: OutcomePoint, kind: OutcomeKind, over: &str) -> String {
    if !p.has {
        return "no data".to_owned();
    }
    match kind {
        OutcomeKind::Rate => format!("{:.0}% ({} of {})", p.value * 100.0, p.n, p.of),
        OutcomeKind::Hours => format!("{:.1} h (over {} {over})", p.value, p.of),
        OutcomeKind::Mean => format!("{:.2} (over {} {over})", p.value, p.of),
        OutcomeKind::Count => p.n.to_string(),
    }
}

fn reads_in_words(reads: OutcomeReads) -> String {
    match reads {
        OutcomeReads::Better => "better".to_owned(),
        OutcomeReads::Worse => "worse".to_owned(),
        OutcomeReads::Same => "the same".to_owned(),
        OutcomeReads::Moved => "moved, neither better nor worse".to_owned(),
        OutcomeReads::Few => format!("too few to tell (under {MIN_EVENTS} events in a window)"),
        OutcomeReads::None => "no figure to compare".to_owned(),
    }
}

fn group_key_word(key: OutcomeGroupKey) -> &'static str {
    match key {
        OutcomeGroupKey::Headline => "headline",
        OutcomeGroupKey::Quality => "quality",
        OutcomeGroupKey::Landing => "landing",
        OutcomeGroupKey::Reliability => "reliability",
    }
}

/// Report r in words for an agent: each KPI with its definition, the three figures to compare and what their move
/// reads as.
#[must_use]
pub fn describe(r: &OutcomeReport) -> String {
    let mut b = String::new();
    for g in &r.groups {
        for k in &g.kpis {
            let better = match k.better {
                OutcomeBetter::Neither => "neither higher nor lower is better",
                OutcomeBetter::Up => "better when higher",
                OutcomeBetter::Down => "better when lower",
            };
            b.push_str(&format!(
                "- {} [{}; {better}]: {} {}, {} {}, all time {}; reads: {}. {}\n",
                k.label,
                group_key_word(g.key),
                r.span.last,
                point_words(k.now, k.kind, &k.over),
                r.span.before,
                point_words(k.before, k.kind, &k.over),
                point_words(k.all, k.kind, &k.over),
                reads_in_words(k.change.reads),
                k.hint,
            ));
        }
    }
    b
}
