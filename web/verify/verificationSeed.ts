// Made-up Verifications, as the server reads a chain's verdicts (crates/orch/src/verification.rs): `judged` is a chain whose
// reviewer asked for changes and then approved, whose QC approved with its proof, and whose goal judge refused the first
// finish and passed the second, with findings (the goal checks are one judge, its rounds the checks, each given by its own card); `judging` has the goal judge at work; `refused` had its first finish
// refused and its Chair back at work; `stale` is a chain whose reviewer and QC approved a change an engineer has
// committed on top of since; `security` is a change that touches a rail, its reviewer and QC approved and its security
// reviewer not yet given a verdict (the board's chip: Awaiting security); `secured` is that change with security approved too
// (Required reviews passed); `none` was judged by nobody. Its goal judges are the cards `<chainId>-judge1` and
// `-judge2`, as the app page seeds them. For the Verification verify page, the app page and their tests.
import type { CheckTone, Finding, Judge, Move, VerdictView, Verification } from '../src/core/generated/types.generated';
import { findingRecord } from './records';

const MIN = 60_000;
const iso = (minAgo: number) => new Date(Date.now() - minAgo * MIN).toISOString();
const verdict = (id: string, cardId: string, minAgo: number, label: string, tone: CheckTone, headline: string, rest = '', findings: Finding[] = [], moves: Move[] = []): VerdictView =>
  ({ id, cardId, check: { label, tone }, headline, rest, more: rest.split('\n').filter((l) => l.trim()).length, findings, moves, at: iso(minAgo) });

const reviewNotes = '## Checked\n- `revenue.go:41` adds tax after the discount, as the goal asks.\n- `revenue_test.go:88` covers a zero-tax order and a 100% discount.\n- `go vet ./...` clean.\n\n## Risks\n- The old rounding is kept for orders before the cut-over date (`revenue.go:57`).';
const qcNotes = '- `go test ./internal/revenue/... -run TestTax -count=3`: ok (0.41s)\n- Replayed the 3 orders from DEMO-5390 against the branch: totals now match the Shopify admin to the cent.\n- `web`: the report page shows the new total after a reload.';
const notMet: Finding[] = [
  findingRecord({ item: 'Totals match the Shopify admin for discounted orders', status: 'met', evidence: 'qc replay of DEMO-5390 orders: 3/3 match (see QC verdict)' }),
  findingRecord({ item: 'The CSV export uses the same total', status: 'not_met', evidence: 'export/csv.go:112 still sums line items without the discount' }),
  findingRecord({ item: 'Merchants are told the history was recomputed', status: 'unverified', blocker: 'person', evidence: 'needs the owner: no notification channel in this repository' }),
];
const met: Finding[] = [
  findingRecord({ item: 'Totals match the Shopify admin for discounted orders', status: 'met', evidence: 'qc replay of DEMO-5390 orders: 3/3 match' }),
  findingRecord({ item: 'The CSV export uses the same total', status: 'met', evidence: 'export/csv.go:112 now calls revenue.Total; export_test.go:40 passes' }),
  findingRecord({ item: 'Merchants are told the history was recomputed', status: 'unverified', blocker: 'person', evidence: 'needs the owner: no notification channel in this repository' }),
];

export function verificationSeed(chainId: string, which: 'judged' | 'judging' | 'refused' | 'stale' | 'security' | 'secured' | 'none' = 'judged'): Verification {
  if (which === 'security' || which === 'secured') {
    const secured = which === 'secured';
    return { chainId, rev: 1, gates: [
      { key: 'review', label: 'Review', state: 'approved', line: 'Approved', who: 'Reviewer · store errors', rounds: [true] },
      { key: 'qc', label: 'QC', state: 'approved', line: 'Approved', who: 'QC · store and runtime tests', rounds: [true] },
      secured
        ? { key: 'security', label: 'Security', state: 'approved', line: 'Approved', who: 'Security · authentication rail', rounds: [false, true] }
        : { key: 'security', label: 'Security', state: 'none', line: 'Not asked yet', who: '', rounds: [] },
      { key: 'goal', label: 'Goal check', state: 'none', line: 'Not asked yet', who: '', rounds: [] },
    ], judges: [
      ...(secured ? [{ key: 'security|remit|sec', cardId: 'sec', role: 'security' as const, name: 'Security · authentication rail', latest: verdict('x3', 'sec', 6, 'Approved', 'done', 'Verdict: APPROVE. The owner key is still checked before the store is read; the wrapped error carries no secret.'), earlier: [verdict('x2', 'sec', 30, 'Changes requested', 'needs', 'Verdict: REQUEST CHANGES. `auth.rs:88` logs the request headers in the wrapped error.')] }] : []),
      { key: 'reviewer|remit|rv', cardId: 'rv', role: 'reviewer', name: 'Reviewer · store errors', latest: verdict('x1', 'rv', 40, 'Approved', 'done', 'Verdict: APPROVE. Every store error names its chain.', reviewNotes), earlier: [] },
      { key: 'qc|remit|q', cardId: 'q', role: 'qc', name: 'QC · store and runtime tests', latest: verdict('x0', 'q', 35, 'Approved', 'done', 'QC ran it on HEAD 7a1e9c4.', qcNotes), earlier: [] },
    ] };
  }
  const refusal: Judge = { key: 'judge|checks', cardId: `${chainId}-judge1`, role: 'judge', name: 'Goal judge', latest: verdict('v7', `${chainId}-judge1`, which === 'refused' ? 1 : 45, 'Goal not met', 'needs', '## Not met: the export still sums the old way\n\nThe API and the report are right; the CSV export is not.', 'The Chair asked to finish with "totals fixed everywhere".\n- Ran the export on the replayed orders: totals differ by the discount.\n- `export/csv.go:112` sums line items.', notMet), earlier: [] };
  if (which === 'stale') {
    const earlier = 'Approved an earlier version';
    return { chainId, rev: 1, gates: [
      { key: 'review', label: 'Review', state: 'stale', line: earlier, who: 'Reviewer · orchestrator split', rounds: [false, true] },
      { key: 'qc', label: 'QC', state: 'stale', line: earlier, who: 'QC · package boundaries and the full test suite', rounds: [true] },
      { key: 'goal', label: 'Goal check', state: 'none', line: 'Not asked yet', who: '', rounds: [] },
    ], judges: [
      { key: 'reviewer|mc|rv', cardId: 'rv', role: 'reviewer', name: 'Reviewer · orchestrator split', latest: verdict('s3', 'rv', 50, earlier, 'needs', 'Verdict: APPROVE. `internal/orch` splits along the store, the views and the session; no import cycle.', reviewNotes),
        earlier: [verdict('s1', 'rv', 140, 'Changes requested', 'needs', 'Verdict: REQUEST CHANGES. `views.go` still reaches into the session’s run map.')] },
      { key: 'qc|mc|q', cardId: 'q', role: 'qc', name: 'QC · package boundaries and the full test suite', latest: verdict('s2', 'q', 45, earlier, 'needs', 'QC ran it on HEAD 9c1d0e2: `go test -race ./...` ok, `go vet` clean.', qcNotes), earlier: [] },
    ] };
  }
  if (which === 'none' || which === 'refused') {
    const refused = which === 'refused';
    return { chainId, rev: 1, judges: refused ? [refusal] : [], gates: [
      { key: 'review', label: 'Review', state: 'none', line: 'Not asked yet', who: '', rounds: [] },
      refused ? { key: 'goal', label: 'Goal check', state: 'changes', line: 'Not met', who: 'Goal judge', rounds: [false] } : { key: 'goal', label: 'Goal check', state: 'none', line: 'Not asked yet', who: '', rounds: [] },
    ] };
  }
  const judging = which === 'judging';
  return {
    chainId,
    rev: 1,
    gates: [
      { key: 'review', label: 'Review', state: 'approved', line: 'Approved', who: 'Reviewer · revenue', rounds: [false, true, true] },
      { key: 'qc', label: 'QC', state: 'approved', line: 'Approved', who: 'QC · revenue replay', rounds: [true, true] },
      judging
        ? { key: 'goal', label: 'Goal check', state: 'running', line: 'Checking the result', who: '', rounds: [false], watch: `${chainId}-judge2` }
        : { key: 'goal', label: 'Goal check', state: 'approved', line: 'Goal met', who: 'Goal judge', rounds: [false, true] },
    ],
    judges: [
      judging ? refusal : { ...refusal, cardId: `${chainId}-judge2`, latest: verdict('v8', `${chainId}-judge2`, 12, 'Goal met', 'done', 'The goal is met: discounted orders total correctly in the API, the report and the export.', 'Checked against the three key results of the goal; one needs the owner.', met, [{ item: 'The CSV export uses the same total', from: 'not_met', to: 'met' }]), earlier: [refusal.latest] },
      { key: 'reviewer|svc|rv', cardId: 'rv', role: 'reviewer', name: 'Reviewer · revenue', repo: 'sales-engine', latest: verdict('v4', 'rv', 70, 'Approved', 'done', 'Verdict: APPROVE. The tax now applies after the discount, and the old rounding stays for history.', reviewNotes),
        earlier: [verdict('v1', 'rv', 190, 'Changes requested', 'needs', 'Verdict: REQUEST CHANGES. `revenue.go:41` applies tax before the discount; the goal says after.', '- Swap the order and add a test with a 100% discount.\n- `revenue_test.go` has no zero-tax case.')] },
      { key: 'reviewer|web|rw', cardId: 'rw', role: 'reviewer', name: 'Reviewer · web', latest: verdict('v5', 'rw', 66, 'Approved', 'done', 'Verdict: APPROVE. The report reads the server total; nothing is summed in the browser any more.') , earlier: [] },
      { key: 'qc|svc|q', cardId: 'q', role: 'qc', name: 'QC · revenue replay', repo: 'sales-engine', latest: verdict('v6', 'q', 60, 'Approved', 'done', 'QC ran it on HEAD 4be7716.', qcNotes), earlier: [verdict('v2', 'q', 150, 'Approved', 'done', 'QC ran it on HEAD 1f0c2aa.', '- `go test ./...`: ok')] },
    ],
  };
}
