// The agent tree (AgentTree) on made-up data: a Chair; an explorer whose work two agents build on; a release replaced by a
// second one, from which a run of work goes on agent to agent (review, fix, review again) beside a tester that also builds
// on the explorer; a writer that builds on the last review and the tester and waits on the owner; and below, the cards
// Remit started: a goal check that refused the finish and a second one (running, or on ?status=done the
// one that passed). Two mentors are on cards: Lan helps the tester, and the Chair asked Minh to help the writer, who has not
// answered yet. No server, no sign-in. Open /verify/tree.html; ?status=done shows a finished chain, ?status=waiting one that
// sleeps until its Chair checks back.
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
import { agentStands, placed, stands } from './standingSeed';
import type { Card, Chain, Edge, Help, Mentor } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { cardRecord, chainRead, chainRecord, helpRecord, mentorRecord } from './records';

const status = new URLSearchParams(location.search).get('status');
const done = status === 'done';
const waiting = status === 'waiting'; // the chain sleeps: every session closed
const MIN = 60_000;
const t0 = Date.now() - 50 * MIN;
const iso = (ms: number) => new Date(ms).toISOString();

const card = (id: string, agent: string, name: string, min: number, extra: Partial<Card> = {}): Card =>
  cardRecord({ id, home: 'ch', agent, name, origin: 'hired', model: 'opus', mode: 'auto', cwd: '/', status: 'stopped', plan: [], ctxUsed: 0, ctxSize: 0, turns: 1, queued: 0, createdAt: iso(t0 + min * MIN), updatedAt: iso(t0 + (min + 20) * MIN), rev: 1, ...extra });
const cards = [
  card('chair', 'chair', 'Chair', 0, { origin: 'chair', model: 'fable', effort: 'high', turns: 41, status: done || waiting ? 'stopped' : 'running' }),
  card('explorer', 'researcher', 'Explorer · sales-engine', 1, { model: 'sonnet', effort: 'low' }),
  card('docs', 'researcher', 'Docs · API changes', 3, { model: 'sonnet' }),
  card('release', 'engineer', 'Release · sales-engine', 4, { turns: 0 }),
  card('release2', 'engineer', 'Release · sales-engine (2)', 6, { turns: 128 }),
  card('reviewer', 'reviewer', 'Reviewer · release scope', 12),
  card('tester', 'qc', 'Tester · e2e checkout', 13, { model: 'sonnet', status: done || waiting ? 'stopped' : 'running' }),
  card('fixer', 'engineer', 'Fixer · review notes', 16, { model: 'sonnet' }),
  card('reviewer2', 'reviewer', 'Reviewer · release scope (2)', 20),
  card('writer', 'writer', 'Writer · release notes for the checkout change and the sales-engine rollout', 24, { model: 'haiku', status: done || waiting ? 'stopped' : 'permission' }),
  card('judge1', 'judge', 'Goal judge · check 1', 25, { origin: 'judge'}),
  card('judge2', 'judge', 'Goal judge · check 2', 40, { origin: 'judge', status: done || waiting ? 'stopped' : 'running' }),
];
const edge = (from: string, to: string): Edge => ({ id: from + to, chainId: 'ch', from, to, kind: 'handoff', label: 'x', createdAt: iso(t0) });
const edges = [edge('explorer', 'docs'), edge('explorer', 'release'), edge('release', 'release2'), edge('release2', 'reviewer'), edge('release2', 'tester'), edge('explorer', 'tester'),
  edge('reviewer', 'fixer'), edge('fixer', 'reviewer2'), edge('reviewer2', 'writer'), edge('tester', 'writer')];
const finished = (id: string) => agentStands('ch', id, done ? 'Done' : 'Stopped', 'done', done ? 'Done · message to ask more' : 'Stopped · message to resume', 'done');
const chairStands = done ? agentStands('ch', 'chair', 'Done', 'done', 'Chain is done', 'done')
  : waiting ? agentStands('ch', 'chair', 'Stopped', 'done', 'Waiting to check back in 2 hours', 'waiting')
  : agentStands('ch', 'chair', 'Running', 'running', 'Waiting for Tester · e2e checkout, Writer · release notes to report', 'waiting');

// Where each agent stands, as the server says it (made up).
const agents = placed('chair', cards, [
  chairStands,
  finished('explorer'),
  finished('docs'),
  agentStands('ch', 'release', 'Error', 'failed', 'Error: start session: the saved context is missing, so Release · sales-engine (2) took over', 'error'),
  finished('release2'),
  agentStands('ch', 'reviewer', 'Awaiting acceptance', 'waiting', 'Reported · asked for 3 changes', 'waiting'),
  done || waiting ? finished('tester') : agentStands('ch', 'tester', 'Running', 'running', 'Running the checkout suite, 14 of 32', 'busy'),
  finished('fixer'),
  finished('reviewer2'),
  done || waiting ? finished('writer') : agentStands('ch', 'writer', 'Needs you', 'needs', 'Approve: publish the release notes', 'needs', ['i1']),
  agentStands('ch', 'judge1', 'Done', 'done', 'Goal not met', 'done', [], { asks: [] }),
  waiting ? agentStands('ch', 'judge2', 'Done', 'done', 'Goal not met', 'done', [], { asks: [] })
    : agentStands('ch', 'judge2', done ? 'Done' : 'Running', done ? 'done' : 'running', done ? 'Goal met' : 'Running a command…', done ? 'done' : 'busy'),
], { docs: 'explorer', release: 'explorer', release2: 'release', reviewer: 'release2', tester: 'explorer', fixer: 'reviewer', reviewer2: 'fixer', writer: 'tester' },
{ tester: ['release2'], writer: ['reviewer2'] });
const chain = chainRecord({ id: 'ch', title: 'resume-by-user checks shop owner', goal: 'x', status: done ? 'done' : waiting ? 'waiting' : 'running', chairId: 'chair', workdir: '/', createdAt: iso(t0), updatedAt: iso(Date.now()), rev: 1 });
// The mentors' helps on the chain, as the server sends them: Lan's accepted, Minh's offered.
const helps: Help[] = [
  helpRecord({ id: 'help_lan', chain: 'ch', card: 'tester', mentor: 'mt_lan', problem: 'The e2e checkout suite flakes on the payment iframe.', state: 'accepted', offeredAt: iso(t0 + 14 * MIN), deadline: iso(t0 + 134 * MIN), answeredAt: iso(t0 + 15 * MIN), rev: 2 }),
  helpRecord({ id: 'help_minh', chain: 'ch', card: 'writer', mentor: 'mt_minh', problem: 'The release notes need the voice the shop owners know.', state: 'offered', offeredAt: iso(t0 + 26 * MIN), deadline: iso(t0 + 146 * MIN), rev: 1 }),
];
const mentors: Mentor[] = ([['mt_lan', 'Lan'], ['mt_minh', 'Minh']] as const).map(([id, label]) => mentorRecord({ id, label, status: 'active', createdAt: iso(t0) }));
fakeServer({
  appSnapshot: () => snapshotOf({ chains: [chainRead(chain)], mentors }),
  chainGet: () => ({ chain: chainRead(chain), cards, edges, retrospectives: [], decisions: [], helps, verification: { chainId: 'ch', gates: [], judges: [], rev: 1 }, datadog: [], landings: [], drafts: [], workflows: [], agents, standing: waiting ? stands('ch', 'Waiting', 'waiting') : stands('ch', 'Running', 'running') }),
  chainEvents: () => ({ events: [], truncated: false }),
});
openChain('ch');

const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/', component: { render: () => null } }] });
createApp({ render: () => h(TooltipProvider, { delayDuration: 300 }, () => h('div', { class: 'flex h-screen flex-col' }, [opened.view && h(AgentTree, { view: opened.view })])) })
  .use(router)
  .provide(apiKey, api)
  .mount('#app');
