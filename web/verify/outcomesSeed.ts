// Made-up Outcomes, shaped like several weeks of a busy owner's chains: every KPI the server defines (crates/outcomes/
// src/kpi.rs) with eight weekly points, the tables, the system now, and the analyst. For the Outcomes verify page and its
// tests. `quiet` is a first week: few figures, no analyst, no insights. The span is the one asked for, in the server's words
// (crates/outcomes/src/span.rs); the points stay the same eight whatever it is.
import type { Decision, OutcomeChange, OutcomeKpi, OutcomePoint, OutcomeRange, OutcomeReads, OutcomeReport, Outcomes, OutcomeSpan, OutcomeWindow } from '../src/core/generated/types.generated';
import { actorOf, decisionRecord } from './records';

const DAY = 86_400_000;
const iso = (ms: number) => new Date(ms).toISOString();
const rate = (n: number, of: number): OutcomePoint => ({ value: of ? n / of : 0, n, of, has: of > 0 });
const count = (v: number): OutcomePoint => ({ value: v, n: v, of: 0, has: true });
const hours = (v: number, of: number): OutcomePoint => ({ value: v, n: 0, of, has: true });
const none: OutcomePoint = { value: 0, n: 0, of: 0, has: false };

function kpi(key: string, label: string, hint: string, kind: OutcomeKpi['kind'], better: OutcomeKpi['better'], series: OutcomePoint[], change: OutcomeChange, all: OutcomePoint, over = ''): OutcomeKpi {
  return { key, label, hint, kind, better, over, series, now: series[series.length - 1], before: series[series.length - 2], all, change };
}

const RANGES: { key: OutcomeRange; days: number; label: string }[] = [
  { key: '7d', days: 7, label: 'Last 7 days' },
  { key: '30d', days: 30, label: 'Last 30 days' },
  { key: '90d', days: 90, label: 'Last 90 days' },
  { key: '180d', days: 180, label: 'Last 180 days' },
  { key: '365d', days: 365, label: 'Last year' },
];
const WINDOWS: { key: OutcomeWindow; days: number; label: string; unit: string; last: string; before: string }[] = [
  { key: '1d', days: 1, label: 'By day', unit: 'day', last: 'the last day', before: 'the day before' },
  { key: '7d', days: 7, label: 'By week', unit: 'week', last: 'the last 7 days', before: 'the 7 days before' },
  { key: '30d', days: 30, label: 'By 30 days', unit: 'month', last: 'the last 30 days', before: 'the 30 days before' },
];

export function spanSeed(last: OutcomeRange = '90d', window: OutcomeWindow = '7d'): OutcomeSpan {
  const r = RANGES.find((x) => x.key === last) ?? RANGES[2];
  const w = WINDOWS.find((x) => x.key === window) ?? WINDOWS[1];
  return {
    range: r.key, window: w.key, last: w.last, before: w.before, unit: w.unit,
    ranges: RANGES.map((x) => ({ key: x.key, label: x.label, fits: w.days < x.days })),
    windows: WINDOWS.map((x) => ({ key: x.key, label: x.label, fits: x.days < r.days })),
  };
}

export function outcomesSeed(quiet = false, last: OutcomeRange = '90d', window: OutcomeWindow = '7d'): Outcomes {
  const now = Date.now();
  const s = <T>(busy: T[], first: T[]) => (quiet ? first : busy);
  const move = (delta: number, reads: OutcomeReads, first: OutcomeReads): OutcomeChange => (quiet ? { delta: 0, reads: first } : { delta, reads });
  const groups: OutcomeReport['groups'] = [
    { key: 'headline', label: 'Loss', kpis: [
      kpi('reverts-per-result', 'Landings taken back per confirmed result', "The landings taken back in the period (by the forge's pipeline, by a guardrail, by a revert they conflicted with, or by the owner), over the results the goal judge confirmed in the same period. A revert is a defect that reached the base; an overturn is one that reached the owner. Read it with the results confirmed beside it: fewer reverts because nothing landed is no gain.", 'mean', 'down',
        s([hours(1.5, 2), hours(0.67, 3), hours(0.5, 4), hours(1, 3), hours(0.4, 5), hours(0.25, 4), hours(0.17, 6), hours(0.25, 4)], [none, none, none, none, none, none, none, hours(0, 1)]), move(0.08, 'few', 'none'), hours(0.45, 33), 'confirmed result'),
      kpi('goal-confirmed', 'Results confirmed', "Of the chains the goal judge has ruled on, those whose latest check approved with every key result met: what the goal promised the customer, shown to hold. An approval that leaves a key result unverified is not counted.", 'rate', 'up',
        s([rate(2, 6), rate(3, 7), rate(4, 8), rate(3, 7), rate(5, 9), rate(4, 8), rate(6, 10), rate(4, 11)], [none, none, none, none, none, none, none, rate(1, 2)]), move(0.1, 'better', 'few'), rate(33, 66)),
    ] },
    { key: 'quality', label: 'Quality', kpis: [
      kpi('accepted-first-time', 'Reports accepted first time', "Of the reports agents handed their Chair, those the Chair accepted without sending back. A report counts once, on the Chair's first word on it.", 'rate', 'up',
        s([rate(14, 20), rate(18, 24), rate(15, 25), rate(20, 26), rate(22, 27), rate(19, 28), rate(23, 29), rate(27, 31)], [none, none, none, none, none, none, none, rate(2, 3)]), move(0.193, 'better', 'none'), rate(158, 210)),
      kpi('goal-first-pass', 'Goal check passed first time', 'Of the chains the goal judge checked, those it passed the first time the Chair asked to finish.', 'rate', 'up',
        s([rate(3, 6), rate(4, 7), rate(5, 8), rate(3, 7), rate(6, 9), rate(5, 8), rate(7, 10), rate(5, 11)], [none, none, none, none, none, none, none, rate(1, 2)]), move(-0.245, 'worse', 'few'), rate(38, 66)),
      kpi('changes-requested', 'Verdicts asking for changes', 'Of the verdicts reviewers and QC gave, those asking for changes. High can mean careful judges or weak work; read it with the first two.', 'rate', 'neither',
        s([rate(9, 30), rate(12, 34), rate(10, 31), rate(14, 38), rate(11, 36), rate(13, 40), rate(12, 41), rate(16, 44)], [none, none, none, none, none, none, none, rate(1, 4)]), move(0.07, 'moved', 'few'), rate(97, 294)),
      kpi('faults-caught-reviewer', 'Faults reviewers caught', 'Of the faults planted on purpose in trial chains that a reviewer ruled on, those its first verdict after the planting asked changes for: the reviewers\' recall on a fault known to be there. A trial chain\'s verdicts count here and in the two below, never in the figures above.', 'rate', 'up',
        s([none, none, none, none, rate(1, 2), rate(2, 3), rate(3, 5), rate(4, 6)], [none, none, none, none, none, none, none, none]), move(0.067, 'better', 'none'), rate(10, 16)),
      kpi('faults-caught-qc', 'Faults QC caught', 'Of the faults planted on purpose in trial chains that a QC ruled on, those its first verdict after the planting asked changes for.', 'rate', 'up',
        s([none, none, none, none, rate(0, 2), rate(1, 3), rate(2, 5), rate(2, 6)], [none, none, none, none, none, none, none, none]), move(-0.067, 'worse', 'none'), rate(5, 16)),
      kpi('faults-caught-judge', 'Faults goal judge caught', 'Of the faults planted on purpose in trial chains that the goal judge ruled on, those its first check after the planting did not pass: the last judge before a change ships, on a fault known to be there.', 'rate', 'up',
        s([none, none, none, none, rate(1, 2), rate(1, 3), rate(2, 4), rate(2, 3)], [none, none, none, none, none, none, none, none]), move(0.167, 'few', 'none'), rate(6, 12)),
    ] },
    { key: 'landing', label: 'Landing', kpis: [
      kpi('queue-wait', 'Wait in the queue', 'How long the landings that reached their base in the period waited from ready (every gate approved) to landed, on average: the time the queue takes to combine, check and push them.', 'hours', 'down',
        s([hours(0.9, 9), hours(0.7, 12), hours(1.4, 10), hours(0.6, 14), hours(0.5, 15), hours(0.45, 13), hours(0.4, 17), hours(0.35, 16)], [none, none, none, none, none, none, none, hours(0.2, 1)]), move(-0.05, 'better', 'none'), hours(0.6, 106), 'landing'),
      kpi('returned', 'Versions sent back by the queue', 'Of the versions the queue landed or sent back to their Chair in the period, those sent back: for a conflict with the base, a check that failed on the combined tree, nothing left to land, a push the forge refused, or a pipeline that failed on it alone.', 'rate', 'down',
        s([rate(3, 12), rate(2, 14), rate(4, 14), rate(2, 16), rate(3, 18), rate(2, 15), rate(1, 18), rate(3, 19)], [none, none, none, none, none, none, none, rate(0, 1)]), move(0.1, 'worse', 'none'), rate(20, 126)),
      kpi('returned-conflict', 'Sent back for a conflict', 'Of the versions the queue landed or sent back in the period, those sent back because they conflict with what landed before them: chains changing the same lines.', 'rate', 'down',
        s([rate(2, 12), rate(1, 14), rate(2, 14), rate(1, 16), rate(2, 18), rate(1, 15), rate(1, 18), rate(2, 19)], [none, none, none, none, none, none, none, rate(0, 1)]), move(0.05, 'moved', 'none'), rate(12, 126)),
      kpi('returned-checks', 'Sent back by the combined checks', 'Of the versions the queue landed or sent back in the period, those whose checks failed once combined with the base: two changes that pass alone and break together.', 'rate', 'down',
        s([rate(1, 12), rate(1, 14), rate(2, 14), rate(1, 16), rate(1, 18), rate(1, 15), rate(0, 18), rate(1, 19)], [none, none, none, none, none, none, none, rate(0, 1)]), move(0.05, 'few', 'none'), rate(8, 126)),
      kpi('batch-size', 'Landings per batch', 'How many landings each push carried, on average. The queue grows a batch by one after one that passes and halves it after one that fails.', 'mean', 'neither',
        s([hours(1.5, 6), hours(2, 6), hours(1.4, 7), hours(2.3, 6), hours(2.5, 6), hours(2.2, 6), hours(2.8, 6), hours(2.7, 6)], [none, none, none, none, none, none, none, hours(1, 1)]), move(-0.1, 'moved', 'none'), hours(2.2, 49), 'batch'),
    ] },
    { key: 'reliability', label: 'Reliability', kpis: [
      kpi('failed-agents', 'Agents that failed', 'Agents whose session ended in a failure (the process died or a turn errored and was never retried).', 'count', 'down',
        s([count(3), count(1), count(4), count(2), count(2), count(5), count(1), count(4)], [none, none, none, none, none, none, none, count(0)]), move(3, 'worse', 'none'), count(22)),
      kpi('goal-not-run', 'Goal checks that could not run', 'Chains that finished without their goal check because the judge failed, was stopped or ran out of time.', 'count', 'down',
        s([count(1), count(0), count(2), count(1), count(0), count(1), count(0), count(0)], [none, none, none, none, none, none, none, count(0)]), move(0, 'same', 'none'), count(5)),
      kpi('stalled', 'Chains finished as stalled', 'Chains Remit finished because nothing moved after repeated reminders.', 'count', 'down',
        s([count(0), count(0), count(1), count(0), count(0), count(0), count(1), count(1)], [none, none, none, none, none, none, none, count(0)]), move(0, 'same', 'none'), count(3)),
    ] },
  ];
  const week = { start: iso(now - 7 * DAY), chains: 0, accepted: 0, sentBack: 0, goalChecked: 0, goalFirstPass: 0, goalNotRun: 0, goalRuled: 0, goalConfirmed: 0, verdicts: 0, changesRequested: 0, landed: 0, batches: 0, queueHours: 0, reverted: 0, overturned: 0, returned: 0, returnedConflict: 0, returnedChecks: 0, failedAgents: 0, stalled: 0, reviewerFaults: 0, reviewerCaught: 0, qcFaults: 0, qcCaught: 0, judgeFaults: 0, judgeCaught: 0 };
  return {
    report: {
      at: iso(now),
      span: spanSeed(last, window),
      periods: Array.from({ length: 8 }, (_, i) => ({ ...week, start: iso(now - (8 - i) * 7 * DAY) })),
      all: week,
      groups,
      roles: quiet ? [] : [
        { role: 'engineer', agents: 64, failed: 9, turns: 412, accepted: 96, sentBack: 31, approved: 0, changesRequested: 0 },
        { role: 'reviewer', agents: 41, failed: 2, turns: 97, accepted: 38, sentBack: 3, approved: 121, changesRequested: 58 },
        { role: 'qc', agents: 39, failed: 4, turns: 110, accepted: 34, sentBack: 5, approved: 104, changesRequested: 39 },
        { role: 'research', agents: 22, failed: 1, turns: 61, accepted: 20, sentBack: 2, approved: 0, changesRequested: 0 },
        { role: 'judge', agents: 66, failed: 5, turns: 66, accepted: 0, sentBack: 0, approved: 38, changesRequested: 28 },
        { role: 'chair', agents: 71, failed: 1, turns: 803, accepted: 0, sentBack: 0, approved: 0, changesRequested: 0 },
      ],
      models: quiet ? [] : [
        { model: 'opus', effort: '', agents: 158, failed: 15, turns: 1350 },
        { model: 'fable', effort: 'high', agents: 71, failed: 1, turns: 803 },
        { model: 'sonnet', effort: '', agents: 145, failed: 7, turns: 199 },
      ],
      kinds: quiet ? [] : [
        { kind: 'work', chains: 38, goalChecked: 36, goalFirstPass: 22, gated: 35, reworked: 17, landed: 29, reverted: 4, landing: 3 },
        { kind: 'chore', chains: 14, goalChecked: 13, goalFirstPass: 7, gated: 13, reworked: 9, landed: 9, reverted: 2, landing: 2 },
        { kind: 'objective', chains: 11, goalChecked: 11, goalFirstPass: 8, gated: 0, reworked: 0, landed: 0, reverted: 0, landing: 0 },
        { kind: 'author', chains: 28, goalChecked: 0, goalFirstPass: 0, gated: 0, reworked: 0, landed: 0, reverted: 0, landing: 0 },
        { kind: 'analyst', chains: 1, goalChecked: 0, goalFirstPass: 0, gated: 0, reworked: 0, landed: 0, reverted: 0, landing: 0 },
      ],
      tools: quiet ? [] : [
        { server: '', name: 'Bash', calls: 4120, failed: 311, agents: 188, chains: 74 },
        { server: '', name: 'Read', calls: 2204, failed: 12, agents: 171, chains: 70 },
        { server: '', name: 'Edit', calls: 903, failed: 41, agents: 96, chains: 52 },
        { server: '', name: 'Grep', calls: 611, failed: 0, agents: 120, chains: 61 },
        { server: '', name: 'WebFetch', calls: 38, failed: 9, agents: 11, chains: 8 },
      ],
      mcp: quiet ? [] : [
        { server: 'remit', name: 'update_plan', calls: 2686, failed: 3, agents: 201, chains: 79 },
        { server: 'remit', name: 'accept_work', calls: 1161, failed: 24, agents: 66, chains: 66 },
        { server: 'claude_ai_Figma', name: 'get_design_context', calls: 17, failed: 5, agents: 4, chains: 3 },
      ],
      skills: quiet ? [] : [
        { server: '', name: 'library:grafana', calls: 14, failed: 0, agents: 9, chains: 6 },
        { server: '', name: 'mongodb', calls: 10, failed: 1, agents: 7, chains: 5 },
        { server: '', name: 'unknown', calls: 1, failed: 0, agents: 1, chains: 1 },
      ],
    },
    analyst: quiet ? null : { chainId: 'ch_a11a57', status: 'waiting', lastRunAt: iso(now - 5 * 3600_000), nextRunAt: iso(now + 19 * 3600_000) },
  };
}

// The analyst's insights, newest first: each a decision of kind insight whose text is the headline and whose why is the
// Markdown body, naming the chains it found it in.
export function insightsSeed(): Decision[] {
  const now = Date.now();
  const insight = (id: string, hoursAgo: number, text: string, why: string): Decision => decisionRecord({ id, chainId: 'ch_a11a57', kind: 'insight', by: actorOf('agent', 'chair'), text, why, createdAt: iso(now - hoursAgo * 3600_000) });
  return [
    insight('d3', 5, 'Goal checks passed first time fell from 70% to 45% (5 of 11): the judge refuses chains whose QC ran only unit tests.',
      'In 4 of the 6 refusals the goal judge found the behaviour never ran end to end:\n\n- ch_9f2c1e: QC ran `go test ./...`, the judge asked for the export to be run against a real file (`export.go:88`).\n- ch_4be771 and ch_d0a3f2: same finding, *"no repro of the reported case"*.\n- ch_51c0aa: the goal named a migration the team never ran.\n\n**What to change:** tell QC in the bugfix playbook to reproduce the reported case before approving, and say which command it ran.'),
    insight('d2', 29, 'Reviewers started catching problems before QC: landed changes needed one QC round fewer than the week before.',
      'Reviewers asked for changes on 16 of 44 verdicts (36%, up from 29%), and the reworked chains still landed: ch_7a1b20, ch_e0c3d9. QC turns per chain fell from 3.1 to 2.4. Nothing to change; worth watching that the rate of changes asked does not keep rising.'),
    insight('d1', 53, 'Four agents failed this week, three of them engineers on opus whose session died mid-turn while a test suite ran.',
      'ch_33aa10, ch_c81f02 and ch_0e9d4b each lost an engineer during `npm test` after 20+ minutes. The Chair resumed two of them; the third chain stalled. Look at the machine\'s memory while long test suites run.'),
  ];
}
