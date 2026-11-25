// The screens with resizable columns (the board's column beside a chain + agent drawer, playbook list, a landing's file tree) on made-up data, to
// drag their edges. No server, no sign-in: a fake server answers the calls of the page's real client core. Open
// /verify/layout.html; ?view=playbooks shows the playbooks screen, ?view=landings the Landings tab; ?card=<id> picks the agent
// in the drawer; ?late=1 has the open chain arrive a moment after the page, as the first snapshot does. The widths are kept
// in localStorage like the real app's.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import { createMemoryHistory, createRouter, RouterView } from 'vue-router';
import { TooltipProvider } from '../src/components/ui/tooltip';
import { apiKey } from '../src/core/api';
import { api } from '../src/replica';
import { routes } from '../src/router';
import { fakeServer, snapshotOf } from './fake';
import type { Card, Chain, FileChange, Playbook } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { boardSeed, stands } from './standingSeed';
import { cardRecord, chainRead, chainRecord, landingRead, landingRecord, playbookRecord, repoRecord, seedRecord } from './records';

const params = new URLSearchParams(location.search);
const view = params.get('view') ?? 'chains';
const late = params.get('late') ? 700 : 0;
const MIN = 60_000;
const t0 = Date.now() - 50 * MIN;
const iso = (ms: number) => new Date(ms).toISOString();

const chain = (id: string, title: string, status: Chain['status'], extra: Partial<Chain> = {}): Chain =>
  chainRecord({ id, title, goal: 'Find the root cause of the empty body {} bug and fix it.', status, chairId: `${id}-chair`, workdir: '/', createdAt: iso(t0), updatedAt: iso(Date.now() - 5 * MIN), rev: 1, ...extra });
const chains = [
  chain('c1', 'resume-by-user checks shop owner', 'running', {
    repos: [repoRecord({ repo: 'g/remit', project: 'g/remit', path: '/', branch: 'feat/x', base: 'main' })],
  }),
  chain('c2', 'A long title that does not fit the column and has to be cut somewhere sensible by the layout', 'waiting'),
  chain('c3', 'Queued: tidy the schedule', 'queued'),
  ...Array.from({ length: 6 }, (_, i) => chain(`d${i}`, `Finished chain number ${i + 1}`, 'done')),
];

const card = (id: string, agent: string, name: string, min: number, extra: Partial<Card> = {}): Card =>
  cardRecord({ id, home: 'c1', agent, name, origin: 'hired', model: 'opus', mode: 'auto', cwd: '/', status: 'stopped', plan: [], ctxUsed: 0, ctxSize: 0, turns: 1, queued: 0, createdAt: iso(t0 + min * MIN), updatedAt: iso(t0 + (min + 20) * MIN), rev: 1, ...extra });
const cards = [
  card('c1-chair', 'chair', 'Chair', 0, { origin: 'chair', status: 'running' }),
  card('investigator', 'researcher', 'Investigate premise and root cause', 1),
  card('engineer', 'engineer', 'Fix: compare parsed int64 after spf_ tables', 5),
];
const files: FileChange[] = [
  { path: 'internal/orch/review.go', added: 4, removed: 1, diff: '@@ -1,2 +1,3 @@\n package orch\n+var x = 1\n import "fmt"\n' },
  { path: 'internal/orch/some_really_long_file_name_that_overflows_the_tree.go', added: 2, removed: 0, diff: '@@ -1,2 +1,3 @@\n package orch\n+var y = 1\n import "fmt"\n' },
  { path: 'README.md', added: 1, removed: 0, diff: '@@ -1,2 +1,3 @@\n # Remit\n+Review the change.\n text\n' },
];
const playbooks: Playbook[] = [
  playbookRecord({ id: 'bugfix', name: 'Bug fix', description: 'Find the root cause, fix it, review it and verify.', group: 'engineering', brief: 'Brief for the Chair.', seeds: [seedRecord({ agent: 'researcher', name: 'Investigate', brief: 'Look first.' })], rev: 1 }),
  playbookRecord({ id: 'feature', name: 'Feature', description: 'Design, build and review a feature end to end across the repos it touches.', group: 'engineering', brief: 'Brief.', seeds: [], rev: 1 }),
];

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
const board = boardSeed({ running: ['c1'], waiting: ['c2'], queued: ['c3'], finished: chains.filter((c) => c.status === 'done').map((c) => c.id) });
fakeServer({
  appSnapshot: async () => (await wait(late), snapshotOf({ chains: chains.map((c) => chainRead(c)), board, standings: chains.map((c) => stands(c.id, c.status, 'running')), playbooks })),
  chainGet: (id) => ({ chain: chainRead(chains.find((c) => c.id === id)!), cards: id === 'c1' ? cards : [], edges: [], retrospectives: [], decisions: [], datadog: [], landings: id === 'c1' ? [landingRead(landingRecord({ id: 'ln1', chainId: 'c1', repo: 'g/remit', version: { number: 1, title: 'fix: shop owner' }, state: { kind: 'ready' }, createdAt: iso(t0) }))] : [], drafts: [], workflows: [], agents: [], helps: [], verification: { chainId: id, gates: [], judges: [], rev: 1 }, standing: stands(id, 'Running', 'running') }),
  chainEvents: () => ({ events: [], truncated: false }),
  cardEvents: () => ({ events: [], truncated: false }),
  landingDiff: (landingId) => ({ landingId, from: 'abc', files }),
});

const router = createRouter({ history: createMemoryHistory(), routes });
await router.push(
  view === 'playbooks' ? { name: 'playbooks' } : { name: 'chain', params: { id: 'c1', tab: view === 'landings' ? 'landings' : 'tree' }, query: { card: params.get('card') ?? undefined } },
);
// The app's top bar is 44px tall; the view fills the rest.
createApp({ render: () => h(TooltipProvider, { delayDuration: 300 }, () => h('div', { class: 'flex h-full flex-col' }, [h('div', { style: 'height:44px;flex:none' }), h(RouterView)])) })
  .use(router)
  .provide(apiKey, api)
  .mount('#app');
