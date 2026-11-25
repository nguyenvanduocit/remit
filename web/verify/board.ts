// The board (BoardView) on made-up data shaped like a busy owner's: what waits for him is only what no agent decides for him
// (ADR-0087: a credential, a sign-in, a permission the Assistant left to him, a failure), many finished chains whose change
// waits in its repository's merge queue (Waiting), many done (two of them new since you looked), 45 put away (two pages of the
// server's), six chains in the Watching column (landed, their Chairs watch a signal), the Finished column's chains Done or Not
// done. No server, no sign-in: a fake client answers the calls, through the same sync as the real app. Open
// /verify/board.html; ?quiet=1 puts one chain in every column instead (and a second in Waiting, on people alone), ?each=1 one
// chain in every column with long reasons and titles (a failure, changes in the merge queue, a check-back, a chain that waits on many people and
// holds no check-back, a watch, a held start, and how a finished chain ended: Done; Not done at the attempt limit, Not done
// with a long list of what it did not prove, and an old chain once "Closed unverified" that the server now reads Not done),
// ?key=1 leaves only a credential waiting, and ?left=1 adds what a deleted chain left on disk, which waits for the owner in the
// first column. A card links to its chain (/chains/<id>); verify/app.html shows the chain's page with its column beside it.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import { createMemoryHistory, createRouter } from 'vue-router';
import BoardView from '../src/components/BoardView.vue';
import { TooltipProvider } from '../src/components/ui/tooltip';
import { apiKey } from '../src/core/api';
import { api } from '../src/replica';
import { fakeServer, snapshotOf } from './fake';
import { routes } from '../src/router';
import { boardSeed, itemWait, needs, stands } from './standingSeed';
import type { Chain, ChainStanding, CheckBack, Column, InboxItem, Wait } from '../src/core/generated/types.generated';
import { chainRead, chainRecord, inboxRecord, optionOf, repoRecord } from './records';
import '../src/theme.css';

const params = new URLSearchParams(location.search);
const each = params.get('each') === '1';
const quiet = params.get('quiet') === '1';
const onlyKey = params.get('key') === '1';
// ?many=<n> adds n more finished chains to Finished, for a column of hundreds.
const many = Number(params.get('many') ?? 0);
const MIN = 60_000;
const iso = (ms: number) => new Date(ms).toISOString();

// A Chair that watches a signal until the effect of its landed change is verified: its deadline in hours, and the wake at it.
const watching = (chainId: string, i: number): CheckBack => ({ at: iso(Date.now() + (i + 1) * 90 * MIN), setAt: iso(Date.now()), why: 'The error rate stays under 2% for a day.',
  watch: { key: `chain:${chainId}.sensor.checkout-error-rate`, condition: 'value.rate >= 0.02', until: iso(Date.now() + (i + 1) * 90 * MIN), why: 'The error rate stays under 2% for a day.' } });
let n = 0;
// When each waiting chain's Chair comes back, by chain id: the server reads it with the chain (ChainRead).
const checkBacks = new Map<string, CheckBack>();
const chain = (title: string, status: Chain['status'], { checkBack, ...extra }: Partial<Chain> & { checkBack?: CheckBack } = {}): Chain => {
  const c = chainRecord({ id: `c${n++}`, title, goal: 'Find the root cause and fix it.', summary: 'Landed in g/r, with a test that fails before the fix and passes after.', status, chairId: 'chair', workdir: '/', createdAt: iso(Date.now() - (60 + n) * MIN), updatedAt: iso(Date.now() - n * 7 * MIN), ...extra });
  if (checkBack) checkBacks.set(c.id, checkBack);
  return c;
};
// A repository the chain works in. The chains `mr…` and the newest put away have their change in its merge queue (ADR-0114).
const inQueue = { has: (id: string) => /^mrs?\d*$/.test(id) || id === 'arch0' };
const withRepo = (repo = 'g/r') => ({ repos: [repoRecord({ repo, project: repo, path: '/', branch: 'fix/x', base: 'main' })] });
const count = (many: number) => (quiet ? 1 : many);
const ask = (id: string, chainId: string, kind: InboxItem['kind'], title: string, detail: string, waitedMin = 0): InboxItem =>
  inboxRecord({ id, home: chainId, cardId: chainId + '-chair', kind, title, detail, status: 'open', createdAt: iso(Date.now() - waitedMin * MIN) });

let chains: Chain[] = [
  ...(quiet ? [] : [chain('sales-engine: the Engineer cannot start', 'running', { id: 'stuck' }), chain('Dashboard totals drift after a refund', 'running', { id: 'stuck2' }),
    chain('Retention of old events', 'running', { id: 'perm' }), chain('Hire a data analyst for the growth objective', 'running', { id: 'apply' }),
    chain('Read the orders from StarRocks', 'running', { id: 'key' }), chain('Weekly Amplitude digest', 'running', { id: 'signin' }),
    chain('Refund webhook retries forever', 'running', { id: 'decide' }), chain('Cut the sync timeout to 30 seconds', 'done', { id: 'live0', ...withRepo() })]),
  ...Array.from({ length: count(29) }, (_, i) => chain(`sales-engine: backfill misses the install-day orders (DEMO-${5385 + i})`, 'done', { id: `mr${i}`, updatedAt: iso(Date.now() - (i + 1) * 3 * MIN), ...withRepo() })),
  ...Array.from({ length: count(19) }, (_, i) => chain(`Scout · 25/10 ${i}:31`, 'done', { id: `${i % 3 ? 'ok' : 'unv'}${i}` })),
  ...Array.from({ length: many }, (_, i) => chain(`Routine · nightly import check ${i}`, 'done', { id: `many${i}` })),
  ...Array.from({ length: quiet ? 1 : 6 }, (_, i) => chain(`Checkout error rate stays low after the coupon change ${i + 1}`, 'waiting', { id: `watch${i}`, checkBack: watching(`watch${i}`, i) })),
  ...(quiet ? [chain('A running chain', 'running'), chain('Checking back later', 'waiting', { checkBack: { at: iso(Date.now() + 30 * MIN), why: 'CI', setAt: iso(Date.now()) } }),
    chain('Refunds are idempotent', 'waiting', { id: 'people' }), chain('Queued chain', 'queued')] : []),
];
// The people at work in a chain that waits on their replies alone, by chain id, as the server names them (orch/standing.go).
const WAITS_ON: Record<string, string> = { people: 'Lan, Minh', crowd: 'Lan Nguyen, Minh Tran, Hoa Pham, Duc Le, Thao Vu, Quang Ho' };
let inbox: InboxItem[] = quiet ? [] : [
  ask('s1', 'stuck', 'error', 'Engineer failed: spawn claude ENOENT', 'Error: spawn claude ENOENT', 45 * 60),
  ask('s3', 'perm', 'permission', 'Bash psql -c "DELETE FROM events WHERE created_at < now() - interval \'90 days\'"', 'Engineer wants to run psql on staging.', 12),
  ask('s5', 'key', 'credential', 'StarRocks needs a password for connection prod-read', 'Type it in the field.', 30),
  ask('s6', 'signin', 'login', 'Sign in to Amplitude for the growth project', 'Open the link and paste the code.', 8),
  ask('a1', 'stuck', 'error', 'Assistant failed: its Claude account hit the weekly limit', 'Error: weekly limit reached', 5),
  // A busy chain's failures: a third of the chains whose change is in the queue failed a turn, the oldest first.
  ...Array.from({ length: 10 }, (_, i) => ask(`f${i}`, `mr${i * 3}`, 'error', `Engineer failed: the checks of !${60 + i * 3} timed out`, 'Error: the checks run past 30 minutes', 20 + i * 111)),
];
// What waits when only a credential does (?key=1): the Needs you column and the palette hold one wait.
if (onlyKey) inbox = inbox.filter((i) => i.kind === 'credential');
if (each) {
  chains = [
    chain('Unit tests for the sync handlers', 'running', { id: 'ask', chairId: 'ask-chair' }),
    chain('Taboola spend: no fake values on a bad rate', 'done', { id: 'mr', ...withRepo('g/handle-sync-taboola-ad') }),
    chain('Two repositories, both changes in their merge queues', 'done', { id: 'mrs', repos: [...withRepo('g/r').repos, ...withRepo('g/s').repos] }),
    chain('Backfill misses the install-day orders', 'running', { id: 'run', chairId: 'run-chair' }),
    chain('Dashboard totals drift after a refund', 'waiting', { id: 'wait', chairId: 'wait-chair', checkBack: { at: iso(Date.now() + 25 * MIN), why: 'The forge\'s pipeline on the landed commit runs for another 20 minutes; the verdict decides whether the Reviewer has to look again.', setAt: iso(Date.now()) } }),
    chain('Payouts settle once per order, reviewed by the whole payments team before the release on Friday', 'waiting', { id: 'crowd', chairId: 'crowd-chair' }),
    chain('A deploy to staging and its error rate, watched for a week by a Chair whose title runs on and on until it needs two lines', 'waiting', { id: 'watch', chairId: 'watch-chair', checkBack: watching('watch', 40) }),
    chain('Queued chain', 'queued', { id: 'queue' }),
    chain('Held until the merge queue drains', 'queued', { id: 'held' }),
    chain('Finished chain, its effect confirmed', 'done', { id: 'ok' }),
    chain('Cut the sync timeout to 30 seconds', 'done', { id: 'live1', ...withRepo() }),
    chain('Refund totals match the dashboard', 'done', { id: 'limit' }),
    chain('Finished chain that did not prove what it claimed and a title that takes two lines', 'done', { id: 'unv' }),
    chain('Set up the remit-dev playbook and its five agents', 'done', { id: 'ch_79e7722eff5f' }),
  ];
  inbox = [
    ask('i2', 'ask', 'error', 'Engineer failed: spawn claude ENOENT', 'Error: spawn claude ENOENT\n    at ChildProcess._handle.onexit (node:internal/child_process:285:19)\n    at onErrorNT (node:internal/child_process:483:16)\n    at process.processTicksAndRejections (node:internal/process/task_queues:82:21)\n    at one more line\n    at and another\n    at and a seventh line of the stack'),
  ];
}

// What waits for the owner from no chain there is: what a deleted chain left on disk (?left=1).
if (params.get('left')) inbox.push({ ...ask('i9', 'ch_deleted', 'error', 'Files of deleted chain ch_deleted are left', 'worktree of g/svc kept: /work/ch_deleted/svc\n\nRemove them by hand, then dismiss this.'), cardId: '', options: [optionOf('dismiss', 'Dismiss')] });

// Where each chain stands, as the server says it (made up, see standingSeed.ts): its column, its badge and why it stands
// there.
const COLUMN: Record<Chain['status'], Column> = { running: 'running', paused: 'running', waiting: 'waiting', queued: 'queued', done: 'finished' };
// A finished chain whose change waits in its merge queue waits for the queue to land it (ADR-0114).
const seedColumn = (c: Chain): Column => (inbox.some((i) => i.home === c.id && waitsOnOwner(i)) ? 'needs' : checkBacks.get(c.id)?.watch ? 'watching' : inQueue.has(c.id) ? 'waiting' : COLUMN[c.status]);

// What waits for the owner, in the server's one order (orch waiting.rs): what no agent decides for him, oldest first. Who it
// is about and what it asks, in the server's words; the newest three arrived since he last looked.
const WHO: Record<string, string> = { stuck: 'Assistant', perm: 'Engineer', key: 'StarRocks', signin: 'Amplitude', ask: 'Chair' };
const oldestFirst = (a: Wait, b: Wait) => a.since.localeCompare(b.since);
// The server's rule (orch waiting.rs waits_on_owner): a credential, a sign-in code, or an item nobody decides for him (the
// Assistant's own failure, a deleted chain's files). Everything else, an Engineer's failure included, goes to the Assistant.
const OWNER_ONLY = new Set(['a1', 'i9']);
const waitsOnOwner = (i: InboxItem) => OWNER_ONLY.has(i.id) || i.kind === 'credential' || i.kind === 'login';
const waiting = inbox.filter(waitsOnOwner).map((i) => itemWait(i, WHO[i.home] ?? 'Engineer', chains.some((c) => c.id === i.home))).sort(oldestFirst)
  .map((w, i, all) => (i >= all.length - 3 || w.id === 's3' ? { ...w, unseen: true } : w));

const seedStanding = (c: Chain): ChainStanding => {
  const checkBack = checkBacks.get(c.id);
  switch (seedColumn(c)) {
    case 'needs':
      return needs(c.id, waiting);
    case 'running':
      return stands(c.id, 'Running', 'running', { working: true, verification: c.id === 'run' ? { label: 'Goal not met · check 1', tone: 'needs' } : undefined });
    case 'waiting':
      if (inQueue.has(c.id)) return stands(c.id, `Waiting for the merge queue · ${c.repos[0]?.repo}`, 'waiting');
      if (!checkBack) return stands(c.id, `Waiting · on ${WAITS_ON[c.id]}`, 'waiting');
      return stands(c.id, 'Waiting', 'waiting', { checkBackAt: checkBack.at, why: checkBack.why });
    case 'watching':
      return stands(c.id, 'Watching', 'watching', { checkBackAt: checkBack!.at, why: checkBack!.watch!.why });
    case 'queued':
      if (c.id === 'held') return stands(c.id, 'Held for the merge queue', 'queued', { place: 2, why: '6 approved changes wait in the merge queue, and at most 5 may before what Remit starts on its own waits.' });
      return stands(c.id, 'Waiting for its turn', 'queued', { place: 1 });
  }
  // A finished chain is Done or Not done, saying what is missing (orch/standing.go outcomeOf).
  const MISSING = 'the refund webhook updates the order total; the dashboard total equals the sum of the orders after a partial refund; the nightly backfill corrects the orders refunded before the fix; the CSV export shows the refunded amount; no shop reports a drift for a week';
  if (c.id === 'unv') return stands(c.id, 'Not done · not proven: ' + MISSING, 'notdone', { why: MISSING });
  if (c.id === 'limit') return stands(c.id, 'Not done · 2 of the goal\'s results still missing', 'notdone', { why: 'the dashboard total equals the sum of the orders; the backfill ran' });
  // Stored as "Closed unverified" before ADR-0067: its approve left the store unread, which the server reads Not done.
  if (c.id === 'ch_79e7722eff5f') return stands(c.id, 'Not done · not proven: playbook remit-dev exists in the store; the five agent definitions it names', 'notdone');
  // Two finished since the owner last looked: the Finished column says "new" until he views them (ADR-0079).
  return c.id.startsWith('unv') ? stands(c.id, 'Not done · 2 of the goal\'s results still missing', 'notdone') : stands(c.id, 'Done', 'done', { unseen: c.id === 'ok1' || c.id === 'ok2' });
};
const by: Partial<Record<Column, string[]>> = {};
for (const c of chains) if (seedColumn(c) !== 'needs') (by[seedColumn(c)] ??= []).push(c.id);
// The column of what needs the owner holds its chains by their first wait, in the list's order.
by.needs = [...new Set(waiting.flatMap((w) => (w.chainId ? [w.chainId] : [])))];
// The chains put away, newest first, which the snapshot leaves out: the server gives them 20 to a page.
// The newest one put away still has a change in the queue: archived, it waits for no one, and no count holds it (ADR-0078).
const archived = Array.from({ length: quiet ? 1 : 45 }, (_, i) => chain(`Put away ${i + 1}: rename the sync fields`, 'done', { id: `arch${i}`, archivedAt: iso(Date.now() - (i + 1) * 60 * MIN), updatedAt: iso(Date.now() - (i + 1) * 60 * MIN), ...(i === 0 ? withRepo() : {}) }));
const board = boardSeed(by, inbox.filter((i) => !chains.some((c) => c.id === i.home)).map((i) => i.id), archived.length, waiting);
const standings = chains.map(seedStanding);

// The snapshot carries the board and every standing; chainGet is not answered on this page.
fakeServer({
  appSnapshot: () => snapshotOf({ chains: chains.map((c) => chainRead(c, checkBacks.get(c.id))), inbox, board, standings }),
  chainsArchived: (after) => {
    const from = Number(after || 0);
    const page = archived.slice(from, from + 20);
    return { chains: page.map((c) => chainRead(c)), standings: page.map((c) => stands(c.id, 'Done', 'done')), next: from + 20 < archived.length ? String(from + 20) : '' };
  },
});

const stub = { render: () => null };
const router = createRouter({
  history: createMemoryHistory(),
  // The app's routes, so the page's links resolve; this page mounts BoardView itself and renders none of them.
  routes: [{ path: '/', component: stub }, ...routes.filter((r) => r.path !== '/')],
});
// The app's top bar is 44px tall; the board fills the rest.
const app = createApp({ render: () => h(TooltipProvider, { delayDuration: 300 }, () => h('div', { class: 'flex h-full flex-col' }, [h('div', { style: 'height:44px;flex:none' }), h(BoardView)])) })
  .use(router)
  .provide(apiKey, api);
await router.push('/' + location.search);
app.mount('#app');
