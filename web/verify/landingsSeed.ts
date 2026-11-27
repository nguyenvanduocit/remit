// Landings and merge queues for the verify pages (ADR-0114), laid out as the server lays them: a chain's landings oldest
// first (ChainView.landings), each queue's in the order it takes them (being combined or checked, then ready, then being judged,
// each by when it became ready; then what landed, went back or was taken back in the last day, newest first). The awkward
// cases: a busy queue holding every state, a version sent back for a conflict and one by the combined checks, a landing taken
// back by the forge's pipeline and the revert that took it back, a landing being taken back by a revert still being combined, a canary stage, a stage that takes an earlier one back, a push the forge refused on a
// protected base, a title far too long for its card, and a landing migrated from a merge request. A landed one names the
// release that carried it (releasesSeed.ts).
import type { Chain, FileChange, Landing, LandingQueue } from '../src/core/generated/types.generated';
import { actorOf, chainRecord, folderRecord, landingRecord } from './records';

const MIN = 60_000;
const HOUR = 60 * MIN;

// A folder worked in place (ADR-0137): its version lands with no queue, and no branch or base.
const NOTES = '/Users/ana/notes';

export const LONG_TITLE = 'feat(gateway): move every session token refresh behind the gateway so a stale refresh never logs a user out mid-checkout, with the retry budget read from the shop settings';

export function landingsSeed(now = Date.now()) {
  const iso = (ago: number) => new Date(now - ago).toISOString();
  const chain = (id: string, title: string, status: Chain['status'] = 'running'): Chain =>
    chainRecord({ id, title, goal: title, status, chairId: `${id}-chair`, workdir: '/tmp', createdAt: iso(2 * 24 * HOUR), updatedAt: iso(5 * MIN) });
  const chains: Chain[] = [
    chain('ch_billing', 'Retry the billing webhook on 429'),
    chain('ch_login', 'Fix the login timeout'),
    chain('ch_docs', 'Write the onboarding docs'),
    chain('ch_cache', 'Cache the shop currency lookups', 'done'),
    chain('ch_long', 'Move every session token refresh behind the gateway'),
    chain('ch_web', 'Roll the new checkout out to every shop'),
    chain('ch_rel', 'Backport the 429 retry to the November release'),
    chain('ch_quiet', 'Rename the zone table', 'queued'),
    { ...chain('ch_notes', 'Rewrite the plan in the notes folder'), repos: [folderRecord(NOTES, NOTES)] },
  ];

  const sha = (n: number) => `${n.toString(16).padStart(4, '0')}c0ffee9a1b2c3d4e5f60718293a4b5c6d7e8f9`;
  const api = { repo: 'acme/api', queue: { host: 'gitlab.example.com', project: 'acme/api', base: 'main' } };
  const web = { repo: 'acme/web', queue: { host: 'github.com', project: 'acme/web', base: 'main' } };
  const remit = actorOf('remit', 'queue');

  // The billing chain, version by version: sent back for a conflict, landed and taken back by the pipeline (with its revert),
  // now a canary stage being checked; and its change on the web app, still being judged.
  const b1 = landingRecord({ ...api, id: 'ln_b1', chainId: 'ch_billing',
    state: { kind: 'returned', reason: 'conflict', output: 'CONFLICT (content): Merge conflict in internal/billing/webhook.go\nlanded first: ln_c1 "perf(currency): cache the shop currency lookups" (internal/billing/webhook.go:41-58)' },
    version: { number: 1, title: 'fix(billing): retry the webhook on 429 with backoff', from: sha(1), forked: sha(100), verdicts: ['v_r1', 'v_q1'], paths: ['internal/billing/webhook.go'] },
    priority: 'medium', priorityWhy: 'Shops lose payments while it fails.', batch: 'bt_40', onto: sha(101), readyAt: iso(9 * HOUR), createdAt: iso(10 * HOUR), updatedAt: iso(8 * HOUR + 50 * MIN) });
  const b2 = landingRecord({ ...api, id: 'ln_b2', release: 'rel_3', chainId: 'ch_billing',
    state: { kind: 'reverted', revert: 'ln_r2', cause: 'pipeline', by: remit, why: 'integration:billing failed on the push, and passed on the base before it.' },
    version: { number: 2, title: 'fix(billing): retry the webhook on 429 with backoff', from: sha(2), forked: sha(102), verdicts: ['v_r2', 'v_q2'], paths: ['internal/billing/webhook.go', 'internal/billing/webhook_test.go'] },
    priority: 'medium', priorityWhy: 'Shops lose payments while it fails.',
    batch: 'bt_44', onto: sha(103), commit: sha(3), push: sha(4), checks: { state: 'failed', failed: 'integration:billing' }, baseChecks: { state: 'passed', failed: '' },
    readyAt: iso(6 * HOUR), landedAt: iso(5 * HOUR + 40 * MIN), revertedAt: iso(4 * HOUR + 52 * MIN), createdAt: iso(7 * HOUR), updatedAt: iso(4 * HOUR + 52 * MIN) });
  const r2 = landingRecord({ ...api, id: 'ln_r2', release: 'rel_4', chainId: 'ch_billing', state: { kind: 'landed' },
    version: { number: 3, title: 'Revert "fix(billing): retry the webhook on 429 with backoff"', gates: [], from: sha(5), forked: sha(4) },
    reverts: { landing: 'ln_b2', cause: 'pipeline', by: remit, why: 'integration:billing failed on the push, and passed on the base before it.' },
    batch: 'bt_45', onto: sha(4), commit: sha(6), push: sha(6), checks: { state: 'passed', failed: '' }, baseChecks: { state: 'failed', failed: 'integration:billing' },
    readyAt: iso(5 * HOUR), landedAt: iso(4 * HOUR + 52 * MIN), createdAt: iso(5 * HOUR), updatedAt: iso(4 * HOUR + 52 * MIN) });
  const b3 = landingRecord({ ...api, id: 'ln_b3', chainId: 'ch_billing', state: { kind: 'checking' },
    version: { number: 4, title: 'fix(billing): retry the webhook on 429, behind billing_retry_v2',
      description: 'Retries a 429 from the payment provider **three times**, 2 s, 8 s and 30 s apart, and only for shops in `billing_retry_v2`.\n\nThe integration test that failed on version 2 now runs against the provider\'s sandbox with its real rate limit.',
      from: sha(7), forked: sha(6), verdicts: ['v_r3', 'v_q3', 'v_s3'], gates: ['reviewer', 'qc', 'security'],
      paths: ['internal/billing/webhook.go', 'internal/billing/webhook_test.go', 'internal/billing/flags.go'],
      plan: { exposure: '5% of shops, behind billing_retry_v2', guardrail: 'chain:ch_billing.sensor.webhook-5xx', breach: 'value.rate > 0.01', bake: 2 * HOUR * 1e6, rollback: 'Set billing_retry_v2 to 0%', undo: false } },
    priority: 'high', priorityWhy: 'Payments fail for every shop on the provider’s busy hour; the revert put the failures back.',
    batch: 'bt_52', onto: sha(110), readyAt: iso(25 * MIN), createdAt: iso(40 * MIN) });
  const bWeb = landingRecord({ ...web, id: 'ln_bw', chainId: 'ch_billing', state: { kind: 'judging' },
    version: { number: 1, title: LONG_TITLE, from: sha(8), forked: sha(120), paths: ['web/src/billing/Retry.vue', 'deploy/helm/web-values.yaml'] },
    priority: 'low', priorityWhy: 'Nobody sees it until the API change is live.', createdAt: iso(12 * MIN) });

  // The rest of acme/api's queue: another batch member being checked, a revert being combined onto the base (the landing it
  // takes back still landed), three ready (one with a title too long for its card), what landed today, one sent back by the
  // combined checks, and one migrated from the merge request it was merged by.
  const c1 = landingRecord({ ...api, id: 'ln_c1', release: 'rel_2', chainId: 'ch_cache', state: { kind: 'landed' },
    version: { number: 1, title: 'perf(currency): cache the shop currency lookups', from: sha(9), forked: sha(99), verdicts: ['v_r4', 'v_q4'] },
    batch: 'bt_40', onto: sha(101), commit: sha(102), push: sha(102), checks: { state: 'passed', failed: '' }, baseChecks: { state: 'passed', failed: '' },
    readyAt: iso(9 * HOUR + 10 * MIN), landedAt: iso(8 * HOUR + 50 * MIN), createdAt: iso(10 * HOUR), updatedAt: iso(8 * HOUR + 50 * MIN) });
  const c2 = landingRecord({ ...api, id: 'ln_c2', chainId: 'ch_cache', state: { kind: 'checking' },
    version: { number: 2, title: 'perf(currency): drop the stale entries when a shop changes currency', from: sha(10), forked: sha(106), verdicts: ['v_r5', 'v_q5'] },
    batch: 'bt_52', onto: sha(110), readyAt: iso(31 * MIN), createdAt: iso(50 * MIN) });
  const k1 = landingRecord({ ...api, id: 'ln_k1', release: 'rel_5', chainId: 'ch_cache', state: { kind: 'landed' },
    version: { number: 3, title: 'perf(currency): read the exchange rates once a minute', from: sha(19), forked: sha(107), verdicts: ['v_r12', 'v_q12'], paths: ['internal/currency/rates.go'] },
    batch: 'bt_49', onto: sha(107), commit: sha(108), push: sha(108), checks: { state: 'passed', failed: '' }, baseChecks: { state: 'passed', failed: '' },
    readyAt: iso(2 * HOUR + 20 * MIN), landedAt: iso(2 * HOUR), createdAt: iso(3 * HOUR), updatedAt: iso(2 * HOUR) });
  const rk1 = landingRecord({ ...api, id: 'ln_rk1', chainId: 'ch_cache', state: { kind: 'combining' },
    version: { number: 4, title: 'Revert "perf(currency): read the exchange rates once a minute"', gates: [], from: sha(20), forked: sha(108), paths: ['internal/currency/rates.go'] },
    reverts: { landing: 'ln_k1', cause: 'overturn', by: actorOf('assistant'), why: 'The owner says finance books each payment at the rate of its moment; a minute-old rate books the wrong amount.' },
    batch: 'bt_53', onto: sha(110), readyAt: iso(6 * MIN), createdAt: iso(7 * MIN) });
  const login = landingRecord({ ...api, id: 'ln_l1', chainId: 'ch_login', state: { kind: 'ready' },
    version: { number: 1, title: 'fix(auth): keep the session alive across the refresh', from: sha(11), forked: sha(110), verdicts: ['v_r6', 'v_q6'] },
    priority: 'high', priorityWhy: 'Users are logged out mid-checkout.', readyAt: iso(18 * MIN), createdAt: iso(30 * MIN) });
  const docs = landingRecord({ ...api, id: 'ln_d1', chainId: 'ch_docs', state: { kind: 'ready' },
    version: { number: 2, title: 'docs: the first-day page', from: sha(12), forked: sha(110), verdicts: ['v_r7', 'v_q7'] },
    priority: 'low', priorityWhy: 'Read when there is time.', readyAt: iso(9 * MIN), createdAt: iso(14 * MIN) });
  const long = landingRecord({ ...api, id: 'ln_g1', chainId: 'ch_long', state: { kind: 'ready' },
    version: { number: 1, title: LONG_TITLE, from: sha(13), forked: sha(110), verdicts: ['v_r8', 'v_q8'] },
    priority: 'medium', priorityWhy: 'The gateway owns the refresh from now on.', readyAt: iso(2 * MIN), createdAt: iso(20 * MIN) });
  const checksFailed = landingRecord({ ...api, id: 'ln_d0', chainId: 'ch_docs',
    state: { kind: 'returned', reason: 'checks', output: '--- FAIL: TestExamples/list_orders (0.04s)\n    examples_test.go:88: GET /v1/orders?limit=5: want 200, got 401 (the example signs in with the old token header)\nFAIL\tacme/api/docs\t0.412s' },
    version: { number: 1, title: 'docs: the first-day page, with the API examples run as tests', from: sha(14), forked: sha(103), verdicts: ['v_r9', 'v_q9'] },
    batch: 'bt_47', onto: sha(105), readyAt: iso(3 * HOUR), createdAt: iso(4 * HOUR), updatedAt: iso(2 * HOUR + 30 * MIN) });
  const migrated = landingRecord({ ...api, id: 'ln_m1', release: 'rel_1', chainId: 'ch_cache', state: { kind: 'landed' },
    version: { number: 0, title: 'chore(ci): pin the Go toolchain', gates: [], from: sha(15), forked: '' },
    commit: sha(98), reference: 'https://gitlab.example.com/acme/api/-/merge_requests/28', landedAt: iso(20 * HOUR), createdAt: iso(22 * HOUR), updatedAt: iso(20 * HOUR) });

  // acme/web: a canary stage ready behind the one that bakes, the stage that takes an earlier one back, what landed.
  const w1 = landingRecord({ ...web, id: 'ln_w1', chainId: 'ch_web', state: { kind: 'ready' },
    version: { number: 3, title: 'feat(checkout): the new checkout for 25% of shops', from: sha(16), forked: sha(130), verdicts: ['v_r10', 'v_q10'],
      plan: { exposure: '25% of shops', guardrail: 'chain:ch_web.sensor.checkout-errors', breach: 'value.rate >= 0.02', bake: 24 * HOUR * 1e6, rollback: 'Set checkout_v2 to 5%', undo: false } },
    priority: 'medium', priorityWhy: 'The 5% stage baked clean.', readyAt: iso(40 * MIN), createdAt: iso(55 * MIN) });
  const w0 = landingRecord({ ...web, id: 'ln_w0', release: 'rel_w1', chainId: 'ch_web', state: { kind: 'landed' },
    version: { number: 2, title: 'feat(checkout): back to 0% while the coupon bug is fixed', gates: [], from: sha(17), forked: sha(128),
      plan: { exposure: '0%, dark behind checkout_v2', guardrail: 'chain:ch_web.sensor.checkout-errors', breach: 'value.rate >= 0.02', bake: 30 * MIN * 1e6, rollback: 'Set checkout_v2 to 5%', undo: true } },
    commit: sha(129), push: sha(129), checks: { state: 'pending', failed: '' }, baseChecks: { state: 'passed', failed: '' },
    landedAt: iso(3 * HOUR), readyAt: iso(3 * HOUR + 5 * MIN), createdAt: iso(3 * HOUR + 10 * MIN), updatedAt: iso(3 * HOUR) });

  // The release branch, protected against Remit's account.
  const rel = landingRecord({ ...api, queue: { ...api.queue, base: 'release/2026.11' }, id: 'ln_rel', chainId: 'ch_rel',
    state: { kind: 'returned', reason: 'refused', output: 'remote: GitLab: You are not allowed to push code to protected branches on this project.\n ! [remote rejected] 4c0ffee -> release/2026.11 (pre-receive hook declined)' },
    version: { number: 1, title: 'fix(billing): retry the webhook on 429 (backport)', from: sha(18), forked: sha(140), verdicts: ['v_r11', 'v_q11'] },
    batch: 'bt_50', onto: sha(140), readyAt: iso(70 * MIN), createdAt: iso(80 * MIN), updatedAt: iso(60 * MIN) });

  // An attempt of a round its gates approved, waiting for the round's pick (ADR-0115): in no queue until it is picked.
  const contending = landingRecord({ ...web, id: 'ln_wb', chainId: 'ch_web', state: { kind: 'contending' },
    version: { number: 4, title: 'feat(checkout): the new checkout for 25% of shops, attempt B', from: sha(20), forked: sha(130), verdicts: ['v_r13', 'v_q13'] },
    readyAt: iso(15 * MIN), createdAt: iso(20 * MIN) });

  const notes = landingRecord({ id: 'ln_n1', chainId: 'ch_notes', repo: NOTES, queue: undefined, state: { kind: 'landed' },
    version: { number: 1, title: 'docs: second draft of the plan', from: sha(30), forked: sha(29), verdicts: ['v_r20'], gates: ['reviewer'], paths: ['plan.txt', 'extra.txt'] },
    priority: 'low', priorityWhy: 'Nothing waits on the notes.', landedAt: iso(20 * MIN), readyAt: iso(21 * MIN), createdAt: iso(25 * MIN), updatedAt: iso(20 * MIN) });

  // The same folder once it became a Git repository: its approved version is a commit on the branch it is on (ADR-0139).
  const notesCommitted = landingRecord({ id: 'ln_n2', chainId: 'ch_notes', repo: NOTES, queue: undefined, state: { kind: 'landed' }, commit: sha(31),
    version: { number: 2, title: 'docs: the plan, with dates', from: sha(32), forked: sha(30), verdicts: ['v_r21'], gates: ['reviewer'], paths: ['plan.txt'] },
    priority: 'low', priorityWhy: 'Nothing waits on the notes.', landedAt: iso(5 * MIN), readyAt: iso(5 * MIN), createdAt: iso(8 * MIN), updatedAt: iso(5 * MIN) });

  const all = [notes, notesCommitted, migrated, b1, c1, checksFailed, b2, r2, k1, w0, w1, rel, b3, c2, rk1, login, docs, long, bWeb, contending];
  // The revert still on its way, as the server says of it (Landing::taking_back), by the revert.
  const takingBack: Record<string, string> = { [rk1.id]: k1.id };
  // Each repository's queues as the server lays them out: a version still judged or contending is in none.
  const queues: Record<string, LandingQueue[]> = {
    'acme/api': [
      { base: 'main', landings: [c2, b3, rk1, login, docs, long, k1, checksFailed, r2, b2, c1, b1, migrated] },
      { base: 'release/2026.11', landings: [rel] },
    ],
    'acme/web': [{ base: 'main', landings: [w1, w0] }],
  };
  return { chains, queues, takingBack, all };
}

// What each landing changes, as Remit's clone reads it: a busy change in folders, a revert, a change with a binary file and a
// deploy file, and one landed before Remit kept its diffs.
const goLines = Array.from({ length: 60 }, (_, i) => `\tstep${i}(ctx, shop)`);
const webhookGo = `@@ -30,60 +30,66 @@ func (h *Webhook) Handle(ctx context.Context, shop Shop) error {\n${goLines
  .map((l, i) => (i === 8 ? `-\tresp, err := h.client.Post(ctx, shop.URL, body)\n+\tresp, err := retry.Do(ctx, h.backoff, func() (*http.Response, error) {\n+\t\treturn h.client.Post(ctx, shop.URL, body)\n+\t})\n+\tif err != nil {\n+\t\treturn fmt.Errorf("shop %s: %w", shop.ID, err)\n+\t}\n` : ` ${l}\n`))
  .join('')}`;
export function filesOf(landingId: string): FileChange[] {
  if (landingId === 'ln_m1') return [];
  if (landingId === 'ln_r2') return [{ path: 'internal/billing/webhook.go', added: 1, removed: 6, diff: '@@ -38,8 +38,3 @@\n-\tresp, err := retry.Do(ctx, h.backoff, func() (*http.Response, error) {\n-\t\treturn h.client.Post(ctx, shop.URL, body)\n-\t})\n-\tif err != nil {\n-\t\treturn fmt.Errorf("shop %s: %w", shop.ID, err)\n-\t}\n+\tresp, err := h.client.Post(ctx, shop.URL, body)\n \tstep9(ctx, shop)\n \tstep10(ctx, shop)\n' }];
  if (landingId === 'ln_bw') return [
    { path: 'web/src/billing/Retry.vue', added: 12, removed: 0, new: true, diff: '@@ -0,0 +1,12 @@\n+<script setup lang="ts">\n+defineProps<{ tries: number }>();\n+</script>\n+\n+<template>\n+  <p>Retrying the payment ({{ tries }} of 3)…</p>\n+</template>\n+\n+<style scoped>\n+p { color: var(--mut); }\n+</style>\n+\n' },
    { path: 'web/public/retry.png', added: 0, removed: 0, new: true, diff: '' },
    { path: 'deploy/helm/web-values.yaml', added: 1, removed: 1, diff: '@@ -3,3 +3,3 @@\n replicas: 3\n-retryBudget: 0\n+retryBudget: 3\n image: web\n' },
  ];
  return [
    { path: 'internal/billing/webhook.go', added: 7, removed: 1, diff: webhookGo },
    { path: 'internal/billing/webhook_test.go', added: 3, removed: 0, diff: '@@ -12,2 +12,5 @@ func TestWebhook(t *testing.T) {\n \tt.Parallel()\n+\tt.Run("429 is retried", func(t *testing.T) {\n+\t\tassertRetried(t, 3)\n+\t})\n }\n' },
    { path: 'internal/billing/flags.go', added: 2, removed: 0, diff: '@@ -1,2 +1,4 @@\n package billing\n+\n+const RetryFlag = "billing_retry_v2"\n import "fmt"\n' },
    { path: 'CHANGELOG.md', added: 1, removed: 0, diff: '@@ -1,2 +1,3 @@\n # Changelog\n+- A 429 from the payment provider is retried three times.\n \n' },
  ];
}
