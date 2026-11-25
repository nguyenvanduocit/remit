// The At work page (AtWorkView) and an employee's runs (EmployeesView, EmployeeRuns) on made-up data, through the real
// client core over its fake server: the core asks `at_work` on connect and keeps the `atWork` store live from the
// `at_work_updated` push. Seats: one that needs the owner (the oldest wait, three days old), a busy Chair with helpers, many
// workers running (some warm, one on a known wait that does not warm), short turns seconds old, a long idle gap, the
// Assistant, a long chain title and name, a seat the owner may not stop. Eight employees wear the six face shapes and the
// six accessories at hues around the wheel. ?empty=1 has no seat. ?at=/employees/<id> opens an employee: its runs are a
// long list (50 shown, `more`), every outcome, waiting ones, how long each took (the two newest run, the oldest ten have no end); ?runs=none has none. window.push plays a push;
// window.atWork is the seed, window.asked the calls the page made, window.stopped each cardStop with its why.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp } from 'vue';
import { createMemoryHistory } from 'vue-router';
import Shell from '../src/components/Shell.vue';
import { apiKey } from '../src/core/api';
import { api } from '../src/replica';
import { signedIn } from '../src/session';
import { appRouter } from '../src/router';
import { fakeServer, snapshotOf } from './fake';
import { agentsSeed } from './librarySeed';
import { wordedRead } from './employeeSeed';
import { STOP_CHAIR, STOP_NO_CHAIR, STOP_TELLS_CHAIR } from './standingSeed';
import type { AtWork, Chain, EmployeeRead, EmployeeRuns, Face, Run, Seat } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { cardRecord, chainRead, chainRecord, employeeRecord } from './records';

const params = new URLSearchParams(location.search);
const now = Date.now();
const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;
const iso = (ms: number) => new Date(ms).toISOString();

const FACES: Face[] = [
  { shape: 'circle', hue: 12, accessory: 'antenna' },
  { shape: 'square', hue: 72, accessory: 'halo' },
  { shape: 'hexagon', hue: 140, accessory: 'visor' },
  { shape: 'triangle', hue: 200, accessory: 'spark' },
  { shape: 'diamond', hue: 262, accessory: 'leaf' },
  { shape: 'drop', hue: 318, accessory: 'bow' },
  { shape: 'circle', hue: 230, accessory: 'visor' },
  { shape: 'hexagon', hue: 40, accessory: 'spark' },
];
const NAMES = ['Login Chair', 'Ada', 'Theo', 'Iris', 'Marina', 'Bao', 'Linh', 'An exceptionally long employee name that has to be cut somewhere'];
const TEMPLATES = ['chair', 'engineer', 'reviewer', 'research', 'qa', 'security', 'architect', 'engineer'];

const employees: EmployeeRead[] = NAMES.map((label, i) => wordedRead(employeeRecord({
  id: `em_${i}`,
  number: i + 1,
  label,
  template: TEMPLATES[i] ?? 'engineer',
  templateRev: 2,
  loadout: { instructions: 'Work in the area you answer for.', tools: ['Read'], skills: [] },
  title: i === 0 ? 'chair' : 'worker',
  reportsTo: i === 0 ? 'owner' : 'em_0',
  hiredBy: i === 0 ? 'owner' : 'em_0',
  status: 'active',
  ceiling: { effect: 'reversible', tools: ['Read', 'Edit'] },
  scope: i === 0 ? 'user login' : '',
  createdAt: iso(now - 20 * DAY),
  rev: 1,
}), FACES[i] ?? FACES[0]!));

const chain = (id: string, title: string, chairId: string): Chain => chainRecord({
  id, title, goal: title, status: 'running', chairId, workdir: '/tmp', createdAt: iso(now - 2 * DAY), updatedAt: iso(now),
});
const LONG = 'Move every session token refresh behind the gateway so a stale refresh never logs a user out mid-checkout, and prove it on staging';
const chains: Chain[] = [chain('ch_login', 'Fix the login timeout', 'c_chair'), chain('ch_long', LONG, 'c_chair2'), chain('ch_docs', 'Write the onboarding docs', 'c_chair3')];

const seat = (s: Partial<Seat> & Pick<Seat, 'cardId' | 'name'>): Seat => ({
  home: 'ch_login', chainTitle: 'Fix the login timeout', employee: '', label: 'Running', tone: 'running', work: '', doing: '', doingTone: 'busy', warms: true, helpers: [], stop: true, kind: 'agent', stopSays: STOP_TELLS_CHAIR, ...s,
});
const seats: Seat[] = [
  seat({ cardId: 'c_ask', name: 'Ada', employee: 'em_1', label: 'Needs you', tone: 'needs', work: 'Choose the cookie domain', doing: 'Asks you which domain the session cookie is set on', doingTone: 'needs', since: iso(now - 3 * DAY - 2 * HOUR), warms: false }),
  seat({ cardId: 'c_chair', kind: 'chair', stopSays: STOP_CHAIR, name: 'Chair', employee: 'em_0', work: 'Step 3 of 5: review the token refresh fix', doing: 'Reading reports', since: iso(now - 47 * MIN), helpers: ['c_theo', 'c_iris'] }),
  seat({ cardId: 'c_theo', name: 'Theo', employee: 'em_2', work: 'Reproduce the timeout on staging', doing: 'Running a command…', since: iso(now - 12 * MIN) }),
  seat({ cardId: 'c_iris', name: 'Iris', employee: 'em_3', work: 'Trace where the refresh throws', doing: 'Searching the code…', since: iso(now - 9 * 1000) }),
  seat({ cardId: 'c_mar', name: 'Marina', employee: 'em_4', label: 'Running', work: 'Add a test for the expired token', doing: 'Waits for a Claude account (all busy)', doingTone: 'waiting', since: iso(now - 4 * MIN), warms: false }),
  seat({ cardId: 'c_long', home: 'ch_long', chainTitle: LONG, name: 'An exceptionally long agent name that has to be cut somewhere in the card', employee: 'em_7', work: LONG, doing: 'Editing gateway/refresh.ts', since: iso(now - 2 * HOUR - 5 * MIN) }),
  seat({ cardId: 'c_bao', home: 'ch_long', chainTitle: LONG, name: 'Bao', employee: 'em_5', work: 'Check the gateway config for secrets', doing: 'Reading a file…', since: iso(now - 31 * 1000) }),
  seat({ cardId: 'c_as', home: 'as_1', kind: 'assistant', stopSays: STOP_NO_CHAIR, chainTitle: '', name: 'Assistant', label: 'Running', work: 'Answer: what changed in login this week', doing: 'Reading the chains…', since: iso(now - 50 * 1000) }),
  seat({ cardId: 'c_judge', home: 'ch_docs', chainTitle: 'Write the onboarding docs', name: 'Goal judge', label: 'Waiting', tone: 'waiting', work: '', doing: 'Waits for the Chair to accept its verdict', doingTone: 'waiting', since: iso(now - 26 * HOUR), warms: false, stop: false, stopSays: '' }),
  seat({ cardId: 'c_linh', home: 'ch_docs', chainTitle: 'Write the onboarding docs', name: 'Linh', employee: 'em_6', label: 'Waiting', tone: 'waiting', work: 'Draft the first-day page', doing: 'Waits for a permission: Edit docs/onboarding.md', doingTone: 'waiting', since: iso(now - 6 * MIN), warms: false }),
];
const atWork: AtWork = { seats: params.get('empty') ? [] : seats, rev: 1 };

const OUTCOMES = ['accepted', 'sent back', 'failed', 'stopped', 'open'];
const SOURCES = ['owner', 'chair', 'agent', 'remit', 'routine'];
const runsOf = (id: string): EmployeeRuns => {
  const runs: Run[] = params.get('runs') === 'none' ? [] : Array.from({ length: 50 }, (_, i) => ({
    chainId: chains[i % chains.length]!.id,
    chainTitle: chains[i % chains.length]!.title,
    cardId: `c_${id}_${i}`,
    request: i === 1 ? `${LONG}. ${LONG}` : `Brief ${50 - i}: ${['reproduce the timeout', 'review the refresh fix', 'write the test', 'check the docs'][i % 4]}`,
    outcome: i < 2 ? 'open' : OUTCOMES[i % OUTCOMES.length]!,
    source: SOURCES[i % SOURCES.length]!,
    waiting: i === 0 || i === 7,
    // The newest is at its work; the next is open but idle, its card waiting for a word, so it does not run.
    live: i === 0,
    started: iso(now - i * 7 * HOUR - 3 * MIN),
    // An end on most finished runs (from 40 s to two days); none on the two open ones, nor on the oldest, which ended
    // before ends were recorded.
    ...(i >= 2 && i < 40 ? { ended: iso(now - i * 7 * HOUR - 3 * MIN + [40_000, 12 * MIN, 95 * MIN, 2 * 24 * HOUR + 5 * HOUR][i % 4]!) } : {}),
  }));
  return { employee: id, runs, more: runs.length > 0, failures: runs.filter((r) => r.outcome === 'failed').length + 9, waiting: 2 };
};

const stopped: { cardId: string; why: string }[] = [];
const fake = fakeServer({
  appSnapshot: async () => structuredClone(snapshotOf({ employees, chains: chains.map((c) => chainRead(c)), agents: agentsSeed() })),
  atWork: async () => structuredClone(atWork),
  employeeRuns: async (id) => structuredClone(runsOf(id)),
  employeeMemory: async () => ({ memories: [], teachings: [] }),
  cardStop: async (cardId, why) => {
    stopped.push({ cardId, why });
    const s = atWork.seats.find((x) => x.cardId === cardId);
    if (!s) throw new Error('no such seat');
    atWork.seats = atWork.seats.filter((x) => x.cardId !== cardId);
    atWork.rev += 1;
    setTimeout(() => fake.push('atWorkUpdated', structuredClone(atWork)));
    return cardRecord({ id: cardId, home: s.home, name: s.name, origin: 'chair', status: 'stopped', createdAt: iso(now), updatedAt: iso(Date.now()), rev: 2 });
  },
});
signedIn();

const router = appRouter(createMemoryHistory());
await router.push(params.get('at') ?? '/at-work');
createApp(Shell).use(router).provide(apiKey, api).mount('#app');
Object.assign(window, { router, push: fake.push, atWork, stopped, asked: fake.asked, requests: fake.requests });
