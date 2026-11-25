// The Signals and Subscriptions tabs (SignalsView: one list of every signal, narrowed by a search and a source; SubscriptionsView: the
// subscriptions and the latest firings; the one open beside them; ?tab=subscriptions opens the second) under the top bar, on made-up data shaped like a busy owner's. Subscriptions: a check-back held because every slot is taken, one held
// because changes of its key result wait in the merge queue, one overdue by a minute, check-backs in minutes, hours and days (a long title and a long
// reason among them), a paused chain; watches (ADR-0013): a chain that holds two (its sensor with a condition, and its own live
// record that wakes at every reading), one past its deadline and held because every slot is taken, one whose source has gone
// silent, one with a very long key and condition; routines (one that repeats, one whose last run is at work, one that
// waits for it, a once-routine that is over, a paused one, one with a long title); and the subscriptions to an event signal (a tell for each of two chains and a job). Signals: with many readings, one whose newest value is markup and one
// whose value is cut, a long key, a silent one. No server, no sign-in: a fake client answers the calls through the same sync as the
// real app. Open /verify/signals.html; ?empty=1 has nothing; ?underway=1 has a once-routine's run under way;
// ?down=1 holds the routines and the due check-backs because no provider can run agents; ?busy=1 adds many more rows;
// ?many=N adds N subscriptions of every kind spread over the three groups (a board of thousands); ?few=1 keeps five; ?records=N
// gives the watch sub_watch_k1_a N readings and N firings. A button's call shows in the console as `asked: <method> <args>`.
// window.router is the router: /signals/<address> opens a line (sub_w1, rt_1, sub_tell_k1,
// signal:chain:k1.sensor.checkout-errors); window.paintMs is how long the page took from its data arriving to its first frame.
import { isPattern, matchesKey } from '../src/pattern';
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import { createMemoryHistory, createRouter, RouterView } from 'vue-router';
import TopBar from '../src/components/TopBar.vue';
import { TooltipProvider } from '../src/components/ui/tooltip';
import { apiKey } from '../src/core/api';
import { api } from '../src/replica';
import { routes } from '../src/router';
import { fakeServer, snapshotOf } from './fake';
import { signalSourcesSeed, sourceOfKey } from './signalSourcesSeed';
import type { Chain, FiringLine, ReadingLine, Sensor, SignalBoard, SignalDetail, SignalLine, Snapshot, SubscriptionLine } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { markPaint } from './paint';
import { Toaster } from '../src/components/ui/sonner';
import { actorOf, chainRead, chainRecord, eventSignalRecord, firingLine, readingLine, sensorRecord, signalLine as signalLineOf, subscriptionLine } from './records';

const params = new URLSearchParams(location.search);
const MIN = 60_000;
const iso = (ms: number) => new Date(Date.now() + ms).toISOString();

const chain = (id: string, title: string, status: Chain['status']): Chain =>
  chainRecord({ id, title, goal: 'g', status, chairId: `${id}-chair`, workdir: '/', createdAt: iso(-5 * 60 * MIN), updatedAt: iso(-MIN), rev: 1 });
// A line as the fake lays it out before the group the server puts it in (below).
type Line = Omit<Parameters<typeof subscriptionLine>[0], 'group'>;
const checkBack = (id: string, title: string, at: number, what: string, state: SubscriptionLine['state'] = 'waiting', held?: string): [Chain, Line] => [
  chain(id, title, state === 'paused' ? 'paused' : 'waiting'),
  { id: `sub_cb_${id}`, kind: 'check_back', title, chainId: id, source: 'clock', at: iso(at), what, state, control: '', ...(held ? { held } : {}) },
];
// A watch of a chain that waits: the signal it reads, the condition that wakes the Chair (none: every reading) and its deadline.
const watch = (id: string, title: string, sub: string, key: string, condition: string, at: number, what: string, state: SubscriptionLine['state'] = 'waiting', held?: string, lastRead?: number): [Chain, Line] => [
  chain(id, title, 'waiting'),
  { id: `sub_watch_${id}_${sub}`, kind: 'watch', title, chainId: id, source: sourceOfKey(key), key, ...(condition ? { condition } : {}), at: iso(at), what, state, control: '', ...(held ? { held } : {}), ...(lastRead === undefined ? {} : { lastRead: iso(lastRead) }) },
];

const down = !!params.get('down');
const underwayRun = !!params.get('underway');
const noAgents = 'no agent can run: claude: signed out; it starts once one can';
const slots = down ? noAgents : 'all 3 slots are taken';
const review = "2 changes of the same key result's work wait in the merge queue, as many as its share (2): it starts as they land";
const rows: [Chain, Line][] = [
  checkBack('w1', 'Deploy of MR !41 to staging', -4 * MIN, 'CI of MR !41 has finished and the deploy should be live: check the health endpoint and the error rate for the last 10 minutes.', 'held', slots),
  checkBack('w2', 'Backfill the install-day orders for the shops that signed up in September', -90_000, 'The backfill job writes about 40k rows an hour; check that the row count in orders_backfill matches the Shopify count for the three shops named in the goal, then report the difference.', 'held', review),
  checkBack('w3', 'Watch the nightly import', 50 * MIN, 'the import runs at 02:00'),
  checkBack('w4', 'mca: save-ad-insights và các consumer phụ thuộc vào nó nên tên rất dài', 3 * 60 * MIN + 12 * MIN, 'Xem consumer đã xử lý hết hàng đợi chưa'),
  checkBack('w5', 'Data has to build up: compare the weekly totals', 2 * 24 * 60 * MIN + 5 * 60 * MIN, 'A week of data is needed before the totals can be compared.'),
  checkBack('w6', 'Compare the totals after the refund fix', -2 * 60 * MIN, 'Compare the refunds column with the report', 'paused'),
];
const watches: [Chain, Line][] = [
  watch('k1', 'Checkout error rate stays low after the coupon change', 'a', 'chain:k1.sensor.checkout-errors', 'value.rate >= 0.02', 3 * 60 * MIN, 'The coupon change leaves the checkout error rate under 2% for a day.', 'waiting', undefined, -2 * MIN),
  watch('k1', 'Checkout error rate stays low after the coupon change', 'b', 'chain:k1.live', '', 26 * 60 * MIN, 'The change is marked live before the window starts.', 'waiting', undefined, -50 * MIN),
  watch('k2', 'Customers still reach the new onboarding step', 'a', 'chain:k2.sensor.step3-reach', 'value.share < 0.6', -MIN, 'At least 60% of new shops reach step 3 within the first hour.', 'held', slots, -9 * MIN),
  watch('k3', 'Order import lag after the queue change', 'a', 'chain:k3.sensor.import-lag', 'value.p95 > 300', 5 * 60 * MIN, 'The p95 import lag stays under five minutes through the night.', 'held',
    'its sensor has failed 3 reads in a row (last error: connection refused): it waits, and the deadline still wakes the Chair', -40 * MIN),
  watch('k4', 'Refund totals agree between the report and the export after the rounding fix, for every shop currency', 'a', 'chain:k4.sensor.refund-total-mismatch-by-currency-and-shop-group', 'value.mismatches > 0 && value.window == "24h" && value.shop_group in ["new", "trial", "paying"]', 8 * 60 * MIN,
    'No currency shows a mismatch between the refund total on the Dashboard and in the CSV export over a full day, for the new, trial and paying shop groups alike.'),
];
const deployScript = `import json, os, urllib.request

req = urllib.request.urlopen(os.environ["API_BASE"] + "/repos/acme/app/deployments/4821/statuses")
latest = json.load(req)[0]
print(json.dumps({"state": latest["state"], "environment": latest["environment"]}))`;
watches.push(
  watch('k5', 'The staging deploy of MR !41 finishes before the owner is told', 'a', 'chain:k5.sensor.deploy', 'value.state == "success"', 110 * MIN,
    'GitHub has no signal for a deployment status, so the Chair reads it with a script every five minutes and comes back when it says success.', 'waiting', undefined, -3 * MIN),
);
// The script sensor the Chair of k5 made: it writes the key that watch listens to (ADR-0047), and the watch shows it from here.
const sensors: Sensor[] = [
  sensorRecord({ id: 'sn_k5', kind: 'script', name: 'deploy', chain: 'k5', signal: 'chain:k5.sensor.deploy', source: 'github', connection: 'default', script: { language: 'python3', code: deployScript }, everyMinutes: 5,
    by: actorOf('agent', 'k5'), createdAt: iso(-110 * MIN), updatedAt: iso(-110 * MIN), rev: 1 }),
];
const routine = (id: string, title: string, goal: string, cadence: string, at: number, state: SubscriptionLine['state'] = 'waiting', extra: Partial<Line> = {}): Line => ({
  id, kind: 'routine', title, source: 'clock', at: iso(at), what: `${goal} · ${cadence}`, state, control: '', ...extra,
});
const routines: Line[] = [
  routine('rt_1', 'Dependency audit of the API', 'Audit the Go dependencies of the API for known vulnerabilities and report what to upgrade first', 'cron 0 9 * * 1-5', 3 * 60 * MIN, 'waiting', { lastFired: iso(-21 * 60 * MIN) }),
  routine('rt_2', 'Check the nightly import', 'Check that the nightly import finished and the row counts match', 'cron 0 7 * * *', 20 * 60 * MIN, 'waiting', { chainId: 'r2' }),
  routine('rt_3', 'Weekly report for the shops that signed up this month', 'Write the weekly report of the new shops: their plan, their first orders and what blocked the ones with none', 'cron 0 8 * * 1', -3 * MIN, 'held', {
    chainId: 'r3',
    held: 'its last run is still at work: the next starts once it is done',
  }),
  routine('rt_4', 'Look at the migration after the deploy', 'Check the migration finished and nothing is slow', 'once', 26 * 60 * MIN),
  routine('rt_5', 'Báo cáo doanh thu hằng tháng cho các cửa hàng mới đăng ký trong tháng, kèm lý do vì sao có cửa hàng chưa có đơn nào', 'Tổng hợp doanh thu theo tuần', 'cron 0 8 1 * *', -40 * 60 * MIN, 'paused'),
  routine('rt_8', 'Close the launch checklist', 'Tick the launch checklist off once', 'once', 0, 'over', { at: undefined, lastFired: iso(-3 * 24 * 60 * MIN) }),
  ...(down ? [routine('rt_6', 'Check the queue depth', 'Check the queue depth is under the alert line', 'cron */30 * * * *', -2 * MIN, 'held', { held: noAgents })] : []),
  ...(underwayRun ? [routine('rt_7', 'Run once now', 'A run that was asked once', 'once', 0, 'underway', { chainId: 'r7', at: undefined })] : []),
];
// The subscriptions to gitlab's events, one line each as the server lays them out: a tell for each of two chains and a job that
// records the merge request's state.
const eventLine = (id: string, title: string, condition: string, what: string, extra: Partial<Line> = {}): Line => ({ id, kind: 'event', title, source: 'event', key: 'event:gitlab', condition, what, state: 'waiting', control: '', lastRead: iso(-3 * MIN), ...extra });
const hooks: Line[] = [
  eventLine('sub_tell_k1', 'Tell the Chair of Checkout error rate', "body.object_kind == 'note'", 'Tells the Chair of the chain a comment is about, as evidence.', { lastFired: iso(-3 * MIN) }),
  eventLine('sub_tell_w1', 'Tell the Chair of the deploy', "body.object_kind == 'merge_request'", 'Tells the Chair of the chain a merge request is about, as evidence.'),
  // What a pattern hears (ADR-0049): every event signal, and a family narrower than a source's shape.
  eventLine('sub_log_all', 'Keep a count of every event', '', 'Counts each event that comes in, whichever signal it came by.', { key: 'event:*', lastFired: iso(-1 * MIN) }),
  eventLine('sub_hear_g', 'Hear the events of the g signals', '', 'Writes down each event of the signals whose name begins with g.', { key: 'event:g*' }),
  eventLine('sub_run_sync', 'Follow the merge request', "body.object_kind == 'merge_request'", 'Records what the forge says of the merge request.', { lastFired: iso(-70 * MIN) }),
];
const busy = Array.from({ length: params.get('busy') ? 60 : 0 }, (_, i) => checkBack(`b${i}`, `Chain ${i}: check the nightly job`, (6 + i) * 60 * MIN, 'Check the nightly job finished without errors.'));
// A board of thousands: check-backs, watches and routines in turn, one in 9 held, one in 13 paused, one in 50 over.
const many = Number(params.get('many') ?? 0);
const manyLines: [Chain, Line][] = Array.from({ length: many }, (_, i) => {
  const state: SubscriptionLine['state'] = i % 9 === 4 ? 'held' : i % 13 === 7 ? 'paused' : 'waiting';
  const heldWhy = state === 'held' ? (i % 2 ? slots : review) : undefined;
  const at = state === 'held' ? -(i % 40) * MIN : (10 + i) * MIN;
  if (i % 3 === 1) return watch(`m${i}`, `Watch ${i}: the error rate of service ${i % 17} stays under its line after the change`, 'a', `chain:m${i}.sensor.service${i % 17}-errors`, 'value.rate >= 0.02', at + 4 * 60 * MIN, 'The error rate stays under 2% for a day.', state, heldWhy, -(i % 30) * MIN);
  if (i % 3 === 2) return [chain(`m${i}`, `Routine ${i}`, 'running'), { ...routine(`rt_m${i}`, `Routine ${i}: check the import of shop group ${i % 23}`, 'Check that the import finished', 'cron 0 7 * * *', at, i % 50 === 3 ? 'over' : state, { lastFired: iso(-(i + 1) * 7 * MIN), ...(i % 50 === 3 ? { at: undefined } : {}) }) }];
  return checkBack(`m${i}`, `Chain ${i}: check the nightly job of shop ${i % 31}`, at, 'Check the nightly job finished without errors.', state, heldWhy);
});
// Five lines only: one held, a watch, a routine and two subscriptions to the event signal.
const few = !!params.get('few');

const empty = !!params.get('empty');
// What Remit does on its own: a hire that answers a fact, a job on a clock, and one the owner paused.
const automation = (id: string, title: string, what: string, state: SubscriptionLine['state'], extra: Partial<Line> & Pick<Line, 'source'>): Line => ({ id, kind: 'automation', title, what, state, control: '', ...extra });
const automations: Line[] = empty || few ? [] : [
  automation('sub_hire_goal_judge', 'Hire the goal judge', 'When a Chair asks to finish, a judge outside the team’s work checks the result against the goal.', 'waiting', { source: 'fact', key: 'fact:chain.finish-requested', lastRead: iso(-30 * MIN), lastFired: iso(-30 * MIN) }),
  automation('sub_hire_goal_recheck', 'Hire the goal judge again after a merge', 'When the change of a finished chain is merged and its last check left something unverified that a merge or a deploy settles, a judge looks again at what is live.', 'paused', { source: 'fact', key: 'fact:chain.merged' }),
  automation('sub_run_snapshot', 'Snapshot the database', 'Copies the database when the last scheduled snapshot is old enough.', 'waiting', { source: 'clock', at: iso(40 * MIN), lastFired: iso(-20 * MIN) }),
];
const all = empty ? [] : few ? [rows[0], watches[0]] : [...rows, ...watches, ...busy, ...manyLines];
const runs = [chain('r2', 'Check the nightly import · 25/10 07:00', 'running'), chain('r3', 'Weekly report for the shops that signed up this month · 26/10 08:00', 'running'), chain('r7', 'Run once now · 26/10 09:00', 'running')];
// A chain that holds two watches is one chain.
const chains = [...new Map([...all.map(([c]) => c), ...runs, chain('w1', 'Deploy of MR !41 to staging', 'waiting'), chain('k1', 'Checkout error rate stays low after the coupon change', 'waiting')].map((c) => [c.id, c])).values()];

// The server's order (signalboard.go sortLines): held or due first, then what comes next, then what is paused or over; within each,
// what waits for a time soonest first, then what waits for a signal, the newest reading first.
const rank = (l: Line) => (l.state === 'held' || l.state === 'due' ? 0 : l.state === 'paused' || l.state === 'over' ? 2 : 1);
const byTime = (l: Line) => l.kind !== 'watch' && l.kind !== 'event';
const read = (l: Line) => (l.lastRead ? new Date(l.lastRead).getTime() : 0);
// A line due at no time comes before every line due at one, as the server orders none first.
const due = (l: Line) => (l.at ? new Date(l.at).getTime() : Number.MIN_SAFE_INTEGER);
const order = (a: Line, b: Line) =>
  rank(a) - rank(b) || (byTime(a) && byTime(b) ? due(a) - due(b) : byTime(a) ? -1 : byTime(b) ? 1 : read(b) - read(a));
// The groups the server puts a line in by its state (signalboard.go groupOf), and its sentence on what is held back.
const GROUP: Partial<Record<SubscriptionLine['state'], SubscriptionLine['group']>> = { held: 'attention', due: 'attention', paused: 'idle', over: 'idle' };
const subscriptions = empty ? [] : [...all.map(([, l]) => l), ...(few ? [routines[0]] : routines), ...automations, ...(few ? hooks.slice(0, 2) : hooks)].sort(order).map((l) => subscriptionLine({ ...l, group: GROUP[l.state] ?? 'coming' }));

// A signal counts the lines that listen to it, as the server does (signalboard.go signalLines).
const signalLine = (key: string, title: string, rev: number, at: number, cause: string, silent?: string): SignalLine =>
  (signalLineOf({ id: `signal:${key}`, key, source: sourceOfKey(key), title, rev, at: iso(at), cause, subscribers: subscriptions.filter((l) => !!l.key && matchesKey(l.key, key)).length, ...(silent ? { silent } : {}), ...(key === 'event:gitlab' ? { eventSignalId: 'es_1' } : {}) }));
const signals: SignalLine[] = empty ? [] : [
  signalLine('event:gitlab', 'Events of gitlab', 41, -3 * MIN, 'gitlab (door)'),
  signalLine('chain:k1.sensor.checkout-errors', 'Sensor checkout-errors of “Checkout error rate stays low after the coupon change”', 188, -2 * MIN, 'Remit (sensor)'),
  signalLine('chain:k2.sensor.step3-reach', 'Sensor step3-reach of “Customers still reach the new onboarding step”', 12, -9 * MIN, 'Remit (sensor)'),
  signalLine('chain:k1.live', 'Checkout error rate stays low after the coupon change is live', 1, -50 * MIN, 'Remit'),
  signalLine('chain:w1.landing.api', 'Landing of “Retry the billing webhook on 429” in api', 3, -70 * MIN, 'Remit (queue)'),
  signalLine('chain:k3.sensor.import-lag', 'Sensor import-lag of “Order import lag after the queue change”', 77, -40 * MIN, 'Remit (sensor)', 'its sensor has failed 3 reads in a row (last error: connection refused)'),
  signalLine('chain:k4.sensor.refund-total-mismatch-by-currency-and-shop-group', 'Sensor refund-total-mismatch-by-currency-and-shop-group of “Refund totals agree between the report and the export after the rounding fix, for every shop currency”', 0, 0, ''),
];
// ?busy=1 also lists many event signals, more than a source shows on the board.
if (params.get('busy')) for (let i = 0; i < 30; i++) signals.push(signalLine(`event:shop${i}`, `Events of shop${i}`, 30 - i, -(10 + i) * MIN, `shop${i} (door)`));
for (const s of signals) if (!s.rev) delete s.at;
const FIRED: FiringLine[] = [
  firingLine({ at: iso(-3 * MIN), title: 'gitlab', key: 'event:gitlab', action: 'tell', outcome: 'told the Chair of “Checkout error rate stays low after the coupon change” about an event', expired: false }),
  firingLine({ at: iso(-25 * MIN), title: 'gitlab', key: 'event:gitlab', action: 'run', outcome: 'recorded the merge request event of billing', expired: false }),
  firingLine({ at: iso(-40 * MIN), title: 'Remit', key: 'fact:chain.finish-requested', chainId: 'k1', cardId: 'k1-judge', action: 'hire', outcome: 'hired the goal judge', expired: false }),
  firingLine({ at: iso(-56 * MIN), title: 'Dependency audit of the API', chainId: 'r2', action: 'start', outcome: 'started a chain', expired: false }),
  firingLine({ at: iso(-2 * 60 * MIN), title: 'Remit', key: 'fact:chain.finish-requested', chainId: 'ch_since_deleted', cardId: 'c_gone', action: 'hire', outcome: 'hired the goal judge', expired: false }),
  firingLine({ at: iso(-21 * 60 * MIN), title: 'Dependency audit of the API', action: 'start', outcome: 'started a chain', expired: false }),
  firingLine({ at: iso(-26 * 60 * MIN), title: 'Order import lag after the queue change', key: 'chain:k3.sensor.import-lag', action: 'wake', outcome: 'woke the Chair: its deadline came with no reading to fire it', expired: true }),
  firingLine({ at: iso(-27 * 60 * MIN), title: 'Compare the refunds', action: 'wake', outcome: 'woke the Chair', expired: false }),
  firingLine({ at: iso(-30 * 60 * MIN), title: 'gitlab', key: 'event:gitlab', action: 'tell', outcome: 'no chain carries its merge request or branch, so nobody was told', expired: false }),
  firingLine({ at: iso(-31 * 60 * MIN), title: 'Dependency audit of the API', action: 'start', outcome: 'started a chain', expired: false }),
];
// ?busy=1 lists the newest signals only, as the server caps them, and the sources count the rest.
const unlisted = params.get('busy') ? { event: 200, sensor: 40 } : {};
const board: SignalBoard = { sources: signalSourcesSeed(signals, subscriptions, { unlisted }), subscriptions, signals, signalsMore: params.get('busy') ? 240 : 0, held: subscriptions.filter((l) => l.state === 'held').length, rev: 1 };

const MARKUP = '<img src=x onerror="document.title=\'pwned\'"> **not bold** [link](javascript:alert(1)) <script>document.title = "pwned"</script>';
const LONG = JSON.stringify({ rate: 0.031, window: '24h', series: Array.from({ length: 90 }, (_, i) => ({ t: i, v: Math.round(Math.sin(i) * 1000) / 1000 })) });
const records = Number(params.get('records') ?? 0);
const manyFirings: FiringLine[] = Array.from({ length: records }, (_, i) => (firingLine({ at: iso(-(3 + 5 * i) * MIN), title: 'Checkout error rate stays low after the coupon change', key: 'chain:k1.sensor.checkout-errors', action: 'wake', outcome: i % 7 === 6 ? 'woke the Chair: its deadline came with no reading to fire it' : `woke the Chair: reading ${188 - i} met value.rate >= 0.02`, expired: i % 7 === 6 })));
// A landing's readings, newest first (ADR-0114): the forge's pipeline failed on the landed commit, the queue landed version
// 2, and before it the Chair submitted version 1, which the queue sent back for a conflict.
const LANDING_READINGS: ReadingLine[] = [
  readingLine({ rev: 3, at: iso(-70 * MIN), cause: 'Remit (sensor landing)', truncated: false, value: JSON.stringify({ landing: 'ln_b2', state: 'landed', version: 2, commit: '9f1c2ab', checks: { state: 'failed', failed: 'integration:billing' }, verdicts: { reviewer: { verdict: 'approve', name: 'Reviewer · Backend', at: iso(-90 * MIN) }, qc: { verdict: 'approve', name: 'Lan Nguyen', at: iso(-80 * MIN) } }, by: ['e_c_1a2b', 'e_c_3c4d'] }) }),
  readingLine({ rev: 2, at: iso(-75 * MIN), cause: 'Remit (queue)', truncated: false, value: JSON.stringify({ landing: 'ln_b2', state: 'landed', version: 2, commit: '9f1c2ab', checks: { state: 'pending', failed: '' }, by: ['e_c_1a2b', 'e_c_3c4d'] }) }),
  readingLine({ rev: 1, at: iso(-3 * 60 * MIN), cause: 'Remit (queue)', truncated: false, value: JSON.stringify({ landing: 'ln_b1', state: 'returned', version: 1, returned: 'conflict', by: ['e_c_1a2b'] }) }),
];
const readings = (key: string): ReadingLine[] => {
  if (key === 'chain:w1.landing.api') return LANDING_READINGS;
  const n = key === 'chain:k1.sensor.checkout-errors' ? Math.max(20, records) : key === 'event:gitlab' ? 3 : 1;
  return Array.from({ length: n }, (_, i): ReadingLine => {
    const rev = 188 - i;
    if (key === 'chain:k1.sensor.checkout-errors' && i === 0) return readingLine({ rev, at: iso(-2 * MIN), cause: 'Remit (sensor)', value: LONG.slice(0, 600), truncated: true });
    if (key === 'event:gitlab' && i === 0) return readingLine({ rev: 41, at: iso(-3 * MIN), cause: 'gitlab (door)', value: MARKUP, truncated: false, eventId: 'event:gitlab#41' });
    return readingLine({ rev: key === 'event:gitlab' ? 41 - i : rev, at: iso((-2 - 5 * (i + 1)) * MIN), cause: 'Remit (sensor)', value: `{"rate":${(0.011 + i / 1000).toFixed(3)},"n":${400 + i}}`, truncated: false });
  });
};
const signalGet = async (address: string): Promise<SignalDetail> => {
  console.log('asked: signalGet', JSON.stringify(address));
  const line = subscriptions.find((l) => l.id === address);
  const key = line?.key ?? address.replace(/^signal:/, '');
  if (!line && !signals.some((s) => s.id === address)) throw Object.assign(new Error('no subscription'), { code: 'NOT_FOUND' });
  const clock = !!line && !line.key;
  return { title: line?.title ?? signals.find((s) => s.id === address)?.title ?? address, key: clock ? '' : key, readings: clock || isPattern(key) ? [] : readings(key), firings: [...FIRED, ...manyFirings].filter((f) => (clock ? f.title === line!.title : f.key === key)) };
};

const ask = (name: string) => (...args: unknown[]) => {
  console.log('asked:', name, JSON.stringify(args));
  return {};
};
const fake = fakeServer({
  appSnapshot: async () => snapshotOf({ chains: chains.map((c) => chainRead(c)), workload: { running: 3, capacity: 3, pressure: '', queued: 2, waiting: new Set(all.map(([c]) => c.id)).size, nextCheckBack: iso(-4 * MIN), rev: 1 }, signals: board, events: [], feedback: [], sensors, eventSignals: [eventSignalRecord({ id: 'wh_1', name: 'gitlab', by: actorOf('assistant', 'c_assistant'), createdAt: iso(-3 * 24 * 60 * MIN), updatedAt: iso(-3 * 24 * 60 * MIN) })] }),
  signalGet: signalGet as never,
  // The stored rows of a key pattern, newest first: the seeded firings (a scheduled one under schedule:<its subscription>, as the
  // server keys it) and the readings of the seeded signals. A pattern that matches none shows the empty table.
  signalRows: (async (pattern: string, limit: number) => {
    const fired = [...FIRED, ...manyFirings].map((f) => ({ key: f.key ?? `schedule:${subscriptions.find((l) => l.title === f.title)?.id ?? 'sub_since_deleted'}`, firing: f }));
    const read = signals.flatMap((s) => readings(s.key).map((r) => ({ key: s.key, reading: r })));
    const at = (row: { firing?: FiringLine; reading?: ReadingLine }) => row.firing?.at ?? row.reading?.at ?? '';
    return [...fired, ...read].filter((row) => matchesKey(pattern, row.key)).sort((a, b) => at(b).localeCompare(at(a))).slice(0, limit);
  }) as never,
  chainCheckBackNow: ask('chainCheckBackNow'),
  chainPause: ask('chainPause'),
  chainResume: ask('chainResume'),
  routineRun: (id) => (console.log('asked: routineRun', JSON.stringify([id])), chain('r4', 'Run', 'queued')),
  routinePause: ask('routinePause'),
  routineResume: ask('routineResume'),
});

const router = createRouter({ history: createMemoryHistory(), routes });
Object.assign(window, { router });
await router.push({ name: params.get('tab') === 'subscriptions' ? 'subscriptions' : 'signals' });
createApp({ render: () => h(TooltipProvider, { delayDuration: 300 }, () => h('div', { class: 'flex h-screen flex-col' }, [h(TopBar, { toReview: 0 }), h('main', { id: 'main', class: 'flex min-h-0 flex-1 flex-col' }, [h(RouterView)]), h(Toaster)])) })
  .use(router)
  .provide(apiKey, api)
  .mount('#app');
// From the board arriving to the first frame drawn with it, for the scripts that time the page at a size.
markPaint();
