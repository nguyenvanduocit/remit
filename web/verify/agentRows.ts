// The agent tree's rows (AgentTree, AgentRow) as the server's work-first standing words them: each row's line is its work,
// the time since `since`, then `doing`, with a pinging dot while its time counts as work (`warms`) and none on a wait with a
// known reason. A Chair at work on a plan step with helpers; a worker seconds into a short turn; one running but waiting for a
// Claude account (no dot); one that needs the owner for three days; one done; one with a very long brief; each worker an
// employee with its face. No server, no sign-in. Open /verify/agentRows.html.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import { createMemoryHistory, createRouter } from 'vue-router';
import AgentTree from '../src/components/AgentTree.vue';
import { TooltipProvider } from '../src/components/ui/tooltip';
import { opened, openChain } from '../src/openChain';
import { apiKey } from '../src/core/api';
import { api } from '../src/replica';
import { fakeServer, snapshotOf } from './fake';
import { wordedRead } from './employeeSeed';
import type { AgentStanding, Card, EmployeeRead, Face } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { cardRecord, chainRead, chainRecord, employeeRecord } from './records';

const now = Date.now();
const MIN = 60_000;
const iso = (ms: number) => new Date(ms).toISOString();

const FACES: Face[] = [
  { shape: 'hexagon', hue: 140, accessory: 'visor' },
  { shape: 'triangle', hue: 200, accessory: 'spark' },
  { shape: 'diamond', hue: 262, accessory: 'leaf' },
  { shape: 'square', hue: 72, accessory: 'halo' },
  { shape: 'drop', hue: 318, accessory: 'bow' },
];
const employees: EmployeeRead[] = ['Theo', 'Iris', 'Marina', 'Ada', 'Bao'].map((label, i) => wordedRead(employeeRecord({
  id: `em_${i}`, number: i + 2, label, template: 'engineer', templateRev: 1, loadout: { tools: [] }, title: 'worker',
  reportsTo: 'owner', hiredBy: 'owner', status: 'active', createdAt: iso(now - 9 * 86_400_000), rev: 1,
}), FACES[i]!));

const card = (id: string, name: string, extra: Partial<Card> = {}): Card =>
  cardRecord({ id, home: 'ch', agent: 'engineer', name, origin: 'hired', model: 'opus', mode: 'auto', cwd: '/', status: 'running', plan: [], ctxUsed: 0, ctxSize: 0, turns: 3, queued: 0, createdAt: iso(now - 60 * MIN), updatedAt: iso(now), rev: 1, ...extra });
const LONG = 'Move every session token refresh behind the gateway so a stale refresh never logs a user out mid-checkout, and prove it on staging';
const cards = [
  card('chair', 'Chair', { origin: 'chair', agent: 'chair', turns: 41 }),
  card('theo', 'Reproducer · staging timeout', { employee: 'em_0' }),
  card('iris', 'Tracer · refresh throws', { employee: 'em_1', turns: 1 }),
  card('marina', 'Tester · expired token', { employee: 'em_2' }),
  card('ada', 'Decider · cookie domain', { employee: 'em_3', status: 'permission' }),
  card('bao', 'Gateway · refresh move', { employee: 'em_4', turns: 12 }),
  card('done', 'Explorer · login logs', { status: 'stopped' }),
];
const standing = (cardId: string, s: Partial<AgentStanding>): AgentStanding => ({
  cardId, home: 'ch', label: 'Running', tone: 'running', doing: '', doingTone: 'busy', work: '', warms: true, waiting: [], children: [], with: [], peers: [],
  branch: { agents: 0, turns: 0, working: true }, asks: ['message'], stopSays: '', remembers: false, rev: 1, ...s,
});
const agents: AgentStanding[] = [
  standing('chair', { work: 'Step 3 of 5: review the token refresh fix', doing: 'Reading reports', since: iso(now - 47 * MIN), children: ['theo', 'iris', 'marina', 'ada', 'bao', 'done'], branch: { agents: 6, turns: 63, working: true } }),
  standing('theo', { work: 'Reproduce the timeout on staging', doing: 'Running a command…', since: iso(now - 12 * MIN) }),
  standing('iris', { work: 'Trace where the refresh throws', doing: 'Searching the code…', since: iso(now - 9_000) }),
  standing('marina', { work: 'Add a test for the expired token', doing: 'Waits for a Claude account (all busy)', doingTone: 'waiting', since: iso(now - 4 * MIN), warms: false }),
  standing('ada', { label: 'Needs you', tone: 'needs', work: 'Choose the cookie domain', doing: 'Asks you which domain the session cookie is set on', doingTone: 'needs', since: iso(now - 3 * 86_400_000 - 2 * 3_600_000), warms: false }),
  standing('bao', { work: LONG, doing: 'Editing gateway/refresh.ts', since: iso(now - 125 * MIN) }),
  standing('done', { label: 'Done', tone: 'done', doing: 'Reported · accepted', doingTone: 'done', warms: false, asks: [] }),
];
const chain = chainRecord({ id: 'ch', title: 'Fix the login timeout', goal: 'x', status: 'running', chairId: 'chair', workdir: '/', createdAt: iso(now - 60 * MIN), updatedAt: iso(now), rev: 1 });
fakeServer({
  appSnapshot: () => snapshotOf({ chains: [chainRead(chain)], employees }),
  chainGet: () => ({ chain: chainRead(chain), cards, edges: [], retrospectives: [], decisions: [], helps: [], verification: { chainId: 'ch', gates: [], judges: [], rev: 1 }, datadog: [], landings: [], drafts: [], workflows: [], agents,
    standing: { chainId: 'ch', text: 'Running', tone: 'running', working: true, waits: 0, unseen: false, asks: [], from: '', rev: 1 } }),
  chainEvents: () => ({ events: [], truncated: false }),
  atWork: () => ({ seats: [], rev: 1 }),
});
openChain('ch');

const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/', component: { render: () => null } }] });
createApp({ render: () => h(TooltipProvider, { delayDuration: 300 }, () => h('div', { class: 'flex h-screen flex-col' }, [opened.view && h(AgentTree, { view: opened.view })])) })
  .use(router)
  .provide(apiKey, api)
  .mount('#app');
