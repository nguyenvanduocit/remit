// The Objectives page's made-up data: objectives of every state (a draft, an active one, one past its horizon, a closed one, a
// task objective that must stay hidden), key results of every status (one of them retired by an amendment, its chain still running), key results refining
// key results two deep (one with no check, met by its children), a stalled key result, owners and judges (one of them retired), a very long outcome, and the
// chains that work toward them, each with where it stands as the server would say it. `many(n)` adds n more
// objectives of those shapes, so the page can be looked at at volume.
import type { Chain, Check, CheckKind, EmployeeRead, KeyResult, KRStatus, ObjectiveView } from '../src/core/generated/types.generated';
import { GAMES, STOREFRONT } from './projectsSeed';
import { stands } from './standingSeed';
import { chainRecord, employeeRecord, objectiveRecord } from './records';
import { wordedRead } from './employeeSeed';

const DAY = 86_400_000;
const iso = (ms: number) => new Date(ms).toISOString();

export const employeesSeed = (now: number): EmployeeRead[] =>
  ([
    ['em_rina', 3, 'Rina', 'worker', 'active'],
    ['em_judge', 4, 'Goal judge', 'judge', 'active'],
    ['em_tom', 5, 'Tom', 'worker', 'retired'],
  ] as const).map(([id, number, label, title, status]) => wordedRead(
    employeeRecord({ id, number, label, title, status, template: 'engineer', templateRev: 1, reportsTo: 'owner', hiredBy: 'owner', createdAt: iso(now - 9 * DAY) }),
    { shape: 'circle', hue: 210, accessory: 'halo' },
  ));

// A check as the server writes it: every field, the ones its kind does not hold empty.
const check = (c: Partial<Check> & Pick<Check, 'kind'>): Check => ({ how: '', signal: '', condition: '', window: '', repo: '', name: '', reason: '', telemetry: '', incident: '', ...c });
const CHECKS: Record<CheckKind, Check> = {
  reading: check({ kind: 'reading', signal: 'refund_rate', condition: '< 1%', window: '7d', expiry: '2027-01-31T00:00:00Z' }),
  unverifiable: check({ kind: 'unverifiable', reason: 'only the customer can say' }),
  submit: check({ kind: 'submit', how: 'the change lands and its judges read it', reason: 'what is right is a matter of taste' }),
  command: check({ kind: 'command', repo: 'github.com/acme/storefront', name: 'npm run perf' }),
};
const kr = (id: string, text: string, owner = 'owner', judge = 'em_judge', kind: CheckKind = 'reading'): KeyResult => ({ id, text, owner, judge, check: CHECKS[kind], retired: false });

const refining = (parent: string, k: KeyResult): KeyResult => ({ ...k, parent });

// The view as the server sends it: the whole tree beside the objective, whose own list is empty.
function objective(id: string, now: number, extra: Partial<Parameters<typeof objectiveRecord>[0]>, keyResults: KeyResult[], status: Record<string, KRStatus>, stalled: Record<string, string> = {}): ObjectiveView {
  return { objective: objectiveRecord({ id, status: 'active', owner: 'owner', createdAt: iso(now - 3 * DAY), customer: 'players of the game factory', ...extra }), keyResults, status, stalled };
}

const STALLED = 'key result k2 is stalled: its last 3 chains finished without progress (no work landed, no goal met): Retry partial captures (ch_s1); Log the failing captures (ch_s2); Retry partial captures again (ch_s3). It takes no new chain until the Assistant lifts the rail, or the owner asks for one';

export function objectivesSeed(now: number): { views: ObjectiveView[]; chains: Chain[] } {
  const views: ObjectiveView[] = [
    objective('ob_refunds', now, { project: GAMES, outcome: 'Players get their refunds within a day', scope: 'game factory', horizon: { text: 'this quarter', until: iso(now + 50 * DAY) }, owner: 'em_rina' }, [
      kr('k1', 'Refunds are paid within 24 hours', 'em_rina'), kr('k2', 'Refund errors stay under 1% of payments', 'em_tom'), kr('k3', 'Players say they are satisfied with a refund', 'owner', 'owner', 'unverifiable'), kr('k4', 'A support agent never has to chase a refund', 'em_rina', 'em_judge', 'submit'),
      { ...kr('k5', 'Refunds by bank transfer are paid within 24 hours too', 'em_tom'), retired: true },
      refining('k1', kr('k1a', 'Card refunds leave the queue within an hour', 'em_rina', 'em_judge', 'command')),
      refining('k1', { ...kr('k1b', 'Wallet refunds are paid the same day', 'em_rina'), check: undefined }),
      refining('k1b', kr('k1b1', 'The wallet provider confirms a refund within 10 minutes', 'em_rina', 'em_judge', 'command')),
      refining('k1b', kr('k1b2', 'A failed wallet refund is retried until it is paid', 'em_rina', 'em_judge', 'command')),
    ], { k1: 'met', k2: 'not_met', k3: 'unverified', k4: 'unserved', k5: 'not_met', k1a: 'met', k1b: 'unverified', k1b1: 'met', k1b2: 'unverified' }, { k2: STALLED }),
    objective('ob_checkout', now, { project: STOREFRONT, outcome: 'Checkout converts above 3%', scope: 'storefront', horizon: { until: iso(now - 12 * DAY) } }, [
      kr('k1', 'Conversion holds above 3% for 14 days'), kr('k2', 'Page load stays under 2 seconds', 'em_rina', 'em_judge', 'command'), { ...kr('k3', 'The coupon field accepts pasted codes', 'owner', 'owner'), check: undefined },
      refining('k3', kr('k3a', 'A pasted code with spaces around it is accepted', 'owner', 'owner', 'command')),
      refining('k3', kr('k3b', 'A pasted code in lower case is accepted', 'owner', 'owner', 'command')),
    ], { k1: 'not_met', k2: 'met', k3: 'unserved', k3a: 'unserved', k3b: 'unserved' }),
    objective('ob_draft', now, { project: STOREFRONT, status: 'draft', outcome: 'Customers can pay with a bank transfer', scope: 'storefront', horizon: { text: 'next month' } }, [kr('k1', 'A transfer is matched to its order within an hour', 'em_rina', 'em_judge', 'submit')], { k1: 'unserved' }),
    objective('ob_long', now, {
      outcome: 'Every shop owner who connects a store sees their first profit report within ten minutes of connecting, with the cost of goods, shipping and ad spend already matched to the right orders, and never has to open a spreadsheet to make the number believable',
      scope: 'a scope label that is also far too long to fit in its column',
    }, [kr('k1', 'The first report is ready within ten minutes for 95% of new stores, measured from the moment the store connects until the report is readable, over a rolling window of seven days')], { k1: 'unverified' }),
    objective('ob_closed', now, { status: 'closed', outcome: 'Invoices reach the customer', scope: 'billing' }, [kr('k1', 'Every invoice is delivered')], { k1: 'met' }),
    objective('ob_empty', now, { outcome: 'An objective with no key result listed yet', scope: 'billing' }, [], {}),
    objective('ob_task', now, { task: true, outcome: 'A direct request that must stay hidden' }, [kr('k1', 'x')], { k1: 'unserved' }),
  ];
  const served = (id: string, objectiveId: string, keyResult: string, title: string, status: Chain['status'], extra: Partial<Chain> = {}) =>
    chainRecord({ id, title, goal: title, status, serves: { objective: objectiveId, keyResult }, chairId: `${id}-chair`, workdir: '/', createdAt: iso(now - 2 * DAY), updatedAt: iso(now - DAY), rev: 1, ...extra });
  const chains = [
    served('ch_a', 'ob_refunds', 'k1', 'Pay refunds from the queue the same day', 'running'),
    served('ch_g', 'ob_refunds', 'k1a', 'Move card refunds to the fast queue', 'done'),
    served('ch_h', 'ob_refunds', 'k1b1', 'Call the wallet provider’s refund status hook', 'done'),
    served('ch_i', 'ob_refunds', 'k1b2', 'Retry failed wallet refunds with backoff', 'running'),
    served('ch_s1', 'ob_refunds', 'k2', 'Retry partial captures', 'done'),
    served('ch_s2', 'ob_refunds', 'k2', 'Log the failing captures', 'done'),
    served('ch_s3', 'ob_refunds', 'k2', 'Retry partial captures again', 'done'),
    served('ch_k', 'ob_checkout', 'k3a', 'Trim spaces around a pasted coupon code', 'queued'),
    served('ch_b', 'ob_refunds', 'k1', 'Alert when a refund waits for more than 20 hours', 'done'),
    served('ch_c', 'ob_refunds', 'k2', 'Find why refunds fail on partial captures', 'waiting'),
    served('ch_f', 'ob_refunds', 'k5', 'Finish the bank transfer refunds already underway', 'running'),
    served('ch_d', 'ob_checkout', 'k1', 'Trim the checkout form to one screen, with a title long enough to be cut where the row ends', 'queued'),
    served('ch_e', 'ob_checkout', 'k2', 'Cache the product images', 'done', { archivedAt: iso(now - DAY) }),
  ];
  return { views, chains };
}

const SHAPES: KRStatus[] = ['met', 'not_met', 'unverified', 'unserved'];
export function many(count: number, now: number): { views: ObjectiveView[]; chains: Chain[] } {
  const views: ObjectiveView[] = [];
  const chains: Chain[] = [];
  for (let i = 0; i < count; i++) {
    const id = `ob_m${i}`;
    const keys = Array.from({ length: 1 + (i % 3) }, (_, j) => kr(`k${j}`, `Key result ${j} of objective ${i}: ${['a rate stays low', 'a time stays short', 'a count reaches its goal'][(i + j) % 3]}`, i % 4 ? 'em_rina' : 'owner'));
    views.push(objective(id, now, { outcome: `Objective ${i}: ${['reach every new customer', 'keep the books honest', 'make the first run delightful', 'never lose a payment'][i % 4]}`, scope: ['game factory', 'storefront', 'billing'][i % 3], status: i % 11 === 0 ? 'draft' : i % 13 === 0 ? 'closed' : 'active', horizon: { until: iso(now + (i % 5 === 0 ? -5 : 40) * DAY) } }, keys, Object.fromEntries(keys.map((k, j) => [k.id, SHAPES[(i + j) % 4]]))));
    keys.forEach((k, j) => {
      if ((i + j) % 2 === 0) chains.push(chainRecord({ id: `ch_m${i}_${j}`, title: `Chain for ${id} ${k.id}`, goal: 'x', status: 'running', serves: { objective: id, keyResult: k.id }, chairId: 'c', workdir: '/', createdAt: iso(now - DAY), updatedAt: iso(now) }));
    });
  }
  return { views, chains };
}

export const standingsOf = (chains: Chain[]) =>
  chains.map((c) => stands(c.id, { running: 'Running', done: 'Done', waiting: 'Waiting', queued: 'Queued', paused: 'Paused' }[c.status], c.status === 'running' ? 'running' : c.status === 'done' ? 'done' : c.status === 'waiting' ? 'waiting' : 'queued', { working: c.status === 'running' }));
