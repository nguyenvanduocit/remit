// The signed-in app (Shell: top bar, connection banner, health bar, the routed view) with the connection banner in each
// state, on made-up data: no server, no sign-in. Open /verify/connection.html?phase=syncing|stale|live|offline|refused|first|back
// (first: the server cannot be reached before the first snapshot; back: the link goes, then comes back after 2.5 s);
// &why=<reason> sets the reason; &health=1 adds the agents-can't-run bar under it (&health=long: with what the provider said at length), to see the two bars stack and the view
// take what is left; &view=board shows the board instead of a chain; &open=wait holds the chain's answer back (its page and its drawer
// while it opens), &open=lost has the connection fail it (what shows when it could not be opened).
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, watch } from 'vue';
import { createMemoryHistory, createRouter } from 'vue-router';
import Shell from '../src/components/Shell.vue';
import { apiKey } from '../src/core/api';
import { api, coreState } from '../src/replica';
import { opened } from '../src/openChain';
import { routes } from '../src/router';
import { fakeServer, snapshotOf } from './fake';
import { boardSeed, stands } from './standingSeed';
import { verificationSeed } from './verificationSeed';
import type { Chain, ChainView, Snapshot } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { cardRecord, chainRead, chainRecord } from './records';

const q = new URLSearchParams(location.search);
const phase = q.get('phase') ?? 'syncing';
const why = q.get('why') ?? '';
const now = new Date().toISOString();
const chain = (id: string, title: string, status: Chain['status']) =>
  chainRecord({ id, title, goal: 'Find the root cause of the empty body {} bug and fix it.', status, chairId: `${id}-chair`, workdir: '/', origin: 'owner', createdAt: now, updatedAt: now, rev: 1 });
const chains = [chain('c1', 'resume-by-user checks shop owner', 'running'), chain('c2', 'Tidy the schedule', 'queued'), chain('c3', 'Fix the empty body {} bug', 'done')];
const chair = cardRecord({ id: 'c1-chair', home: 'c1', agent: 'chair', name: 'Chair', origin: 'chair', model: 'opus', mode: 'auto', cwd: '/', status: 'running', plan: [], ctxUsed: 0, ctxSize: 0, turns: 1, queued: 0, createdAt: now, updatedAt: now, rev: 1 });
const said = q.get('health') === 'long'
  ? "claude: Internal error: You've hit your weekly limit · resets Oct 30, 3pm (UTC)"
  : 'claude: signed out';
const health = q.get('health') ? { ok: false, error: said, accounts: [], checkedAt: now, unreadable: 0, rev: 1 } : undefined;

const chainAnswer = (id: string): Promise<ChainView> => {
  if (q.get('open') === 'wait') return new Promise(() => {});
  if (q.get('open') === 'lost') return Promise.reject(new Error('the connection was lost'));
  return Promise.resolve({ chain: chainRead(chains.find((c) => c.id === id)!), cards: id === 'c1' ? [chair] : [], edges: [], retrospectives: [], decisions: [], helps: [], verification: verificationSeed(id, 'none'), datadog: [], landings: [], drafts: [], workflows: [], agents: [], standing: stands(id, 'Running', 'running') });
};
const snapshot = (): Snapshot => snapshotOf({ chains: chains.map((c) => chainRead(c)), board: boardSeed({ running: ['c1'], queued: ['c2'], finished: ['c3'] }), standings: [stands('c1', 'Running', 'running', { working: true }), stands('c2', 'Waiting for its turn', 'queued', { place: 1 }), stands('c3', 'Done', 'done')], ...(health && { health }) });
// The first snapshot is answered, so the page holds data; the session then goes under and the core dials again, and the
// snapshot it asks for then never comes (syncing) or fails with `why` (stale, which the core tries again and again).
// offline: the server then cannot be reached, so every dial fails. refused: the server turns the page away as one of an
// older build (new_bundle), which the page is told to reload for.
let snapshots = 0;
const fake = fakeServer({
  appSnapshot: () => {
    if (phase === 'first') return new Promise<Snapshot>(() => {});
    if (snapshots++ === 0 || phase === 'live' || phase === 'back') return snapshot();
    if (phase === 'syncing') return new Promise<Snapshot>(() => {});
    throw new Error(why || 'the snapshot could not be read');
  },
  chainGet: (id) => chainAnswer(id),
  chainEvents: () => ({ events: [], truncated: false }),
  cardEvents: () => ({ events: [], truncated: false }),
  chainsArchived: () => ({ chains: [], standings: [], next: '' }),
});

const router = createRouter({ history: createMemoryHistory(), routes });
await router.push(q.get('view') === 'board' ? { name: 'board' } : { name: 'chain', params: { id: 'c1', tab: 'tree' } });
createApp(Shell).use(router).provide(apiKey, api).mount('#app');
// The session goes under once what the page shows is in: the snapshot, and the chain once it is opened.
if (phase === 'first') fake.goDown();
else if (phase !== 'live') {
  const ready = () => coreState.value?.phase === 'live' && (q.get('view') === 'board' || q.get('open') || opened.view);
  if (!ready()) await new Promise<void>((done) => { const stop = watch(ready, (r) => r && (stop(), done())); });
  if (phase === 'offline') fake.goDown();
  else if (phase === 'back') (fake.goDown(), setTimeout(() => fake.comeUp(), 2500));
  else if (phase === 'refused') fake.refuse('new_bundle', why || 'the server runs a newer build');
  else fake.lose();
}
