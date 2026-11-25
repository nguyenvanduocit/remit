// A chain's page (ChainPage) at volume, on made-up data: ?decisions= decisions (default 500), ?landings= landings across
// as many repositories (default 12), in every state, ?judges= judges of its changes (default 30), each with rounds, and the busy Live feed
// (?events=, default 5000), its links to Datadog (?datadog=0: none). Open /verify/chainMany.html?tab=overview (also landings, verification, live, tree); 5, 500 and
// 5000 are the sizes each tab has to hold. The newest decisions are two duel judges' picks: one the owner overturned, with his
// word under it, and one that stands, whose Overturn answers by adding the overturn as the server would push it. No server, no
// sign-in.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import { createMemoryHistory, createRouter } from 'vue-router';
import ChainPage from '../src/components/ChainPage.vue';
import { TooltipProvider } from '../src/components/ui/tooltip';
import { apiKey } from '../src/core/api';
import { opened, openChain } from '../src/openChain';
import { api } from '../src/replica';
import { routes } from '../src/router';
import { fakeServer, snapshotOf } from './fake';
import { stands } from './standingSeed';
import { verificationSeed } from './verificationSeed';
import type { Actor, Card, DecisionKind, DecisionRead, EventKind, EventRead, Judge, LandingState, VerdictView } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { tool, turnEnd } from './activitySeed';
import { actorOf, cardRecord, chainRead, chainRecord, decisionRead, decisionRecord, landingRead, landingRecord, repoRecord, seedEvent, type EventSeed } from './records';

const params = new URLSearchParams(location.search);
const N = (k: string, d: number) => Number(params.get(k) ?? d);
const tab = params.get('tab') ?? 'overview';
const MIN = 60_000;
const now = Date.now();
const iso = (ms: number) => new Date(ms).toISOString();
const STATES: LandingState[] = [{ kind: 'judging' }, { kind: 'ready' }, { kind: 'combining' }, { kind: 'checking' }, { kind: 'landed' },
  { kind: 'returned', reason: 'conflict', output: '' }, { kind: 'reverted', revert: 'ln0', cause: 'overturn', by: actorOf('owner'), why: '' }];
const repos = Array.from({ length: N('landings', 12) }, (_, i) => repoRecord({ repo: `g/service-${i}`, project: `g/service-${i}`, path: '/', branch: `feat/change-${i}`, base: 'main' }));
const landings = repos.map((r, i) => landingRecord({ id: `ln${i}`, chainId: 'ch', repo: r.repo, state: STATES[i % STATES.length],
  version: { number: 1 + (i % 3), title: `feat(service-${i}): ${i % 2 ? 'a change whose title runs on long enough to need two lines of its card before it is cut' : 'short change'}` }, createdAt: iso(now - (repos.length - i) * 20 * MIN) }));
const chain = chainRecord({ id: 'ch', title: 'A chain with a great deal to show', goal: 'Rework every service so that each wraps its errors with the chain id, reviews it and verifies it on staging.', status: 'running', chairId: 'chair', workdir: '~/.remit/work/ch',
  origin: 'owner', createdAt: iso(now - 3000 * MIN), updatedAt: iso(now), repos, summary: '## So far\nEvery service but two wraps its errors.' });
const card = (id: string, agent: string, name: string, extra: Partial<Card> = {}): Card =>
  cardRecord({ id, home: 'ch', agent, name, origin: agent === 'chair' ? 'chair' : 'hired', model: 'opus', mode: 'auto', cwd: '/', status: 'running', plan: [
    { content: 'Find where errors lose the chain id', priority: 'high', status: 'completed' }, { content: 'Wrap them, service by service', priority: 'high', status: 'in_progress' }, { content: 'Have QC verify on staging', priority: 'medium', status: 'pending' },
  ], ctxUsed: 90_000, ctxSize: 200_000, turns: 40, queued: 0, sessionId: 's', createdAt: iso(now - 3000 * MIN), updatedAt: iso(now), ...extra });
const cards = [card('chair', 'chair', 'Chair'), card('eng', 'engineer', 'Engineer'), card('rv', 'reviewer', 'Reviewer'), card('q', 'qc', 'QC')];
const agent = (who: string): Actor => actorOf('agent', who);
const KINDS: DecisionKind[] = ['hire', 'route', 'accept', 'rework', 'landing', 'ask', 'verdict', 'pause', 'resume'];
// Which of KINDS the server marks an outcome.
const OUTCOME_AT = new Set([4, 7, 8]);
const decisions: DecisionRead[] = Array.from({ length: N('decisions', 500) }, (_, i) => decisionRead(decisionRecord({ id: `d${i}`, chainId: 'ch', kind: KINDS[i % KINDS.length], by: i % 4 ? agent('chair') : actorOf('owner', 'owner@example.com', 'web'), text: `Decision ${i + 1}: ${i % 3 ? 'routed the report to the reviewer' : 'hired an engineer for service-' + (i % 12)}`, why: i % 2 ? 'Because the report said so, and the playbook asks for a review of every change.' : '', createdAt: iso(now - (3000 - i * 5) * MIN) }), OUTCOME_AT.has(i % KINDS.length)));

// Two duel judges' picks, the newest decisions: the first overturned by the owner, the second standing.
const judgeBy = agent('judge');
decisions.push(
  decisionRead(decisionRecord({ id: 'd_pick1', chainId: 'ch', kind: 'duel', by: judgeBy, text: 'Duel du_1 on Wrap errors in service-3: picked the engineer report over the builder report', why: 'It wraps every error at the boundary and its test names the chain id; the builder report leaves two paths unwrapped.', cardId: 'judge', createdAt: iso(now - 40 * MIN) }), false),
  decisionRead(decisionRecord({ id: 'd_ov1', chainId: 'ch', kind: 'overturn', by: actorOf('owner', 'owner@example.com', 'web'), text: 'Overturned: Duel du_1 on Wrap errors in service-3: picked the engineer report over the builder report', why: 'The builder report keeps the old error codes the mobile app reads; the engineer one breaks them.', overturns: 'd_pick1', cardId: 'judge', createdAt: iso(now - 20 * MIN) }), false),
  decisionRead(decisionRecord({ id: 'd_pick2', chainId: 'ch', kind: 'duel', by: judgeBy, text: 'Duel du_2 on Retry the payment webhook: picked the builder report over the engineer report', why: 'Its retry is idempotent on the event id.', cardId: 'judge', createdAt: iso(now - 5 * MIN) }), false),
);

// Where Datadog shows the chain, as the server lays it out with a site and a key set; ?datadog=0: none set, no links.
const dd = (label: string, path: string) => ({ label, url: `https://app.datadoghq.com/${path}?query=%40remit.chain_id%3Ach` });
const datadog = params.get('datadog') === '0' ? [] : [dd('traces', 'apm/traces'), dd('logs', 'logs'), dd('errors', 'error-tracking')];

// Many judges of the changes, after the seed's goal judge: a reviewer and a QC per repository, each with a round or two.
const base = verificationSeed('ch', 'judged');
const round = (id: string, cardId: string, ago: number, ok: boolean): VerdictView => ({ id, cardId, check: ok ? { label: 'Approved', tone: 'done' } : { label: 'Changes requested', tone: 'needs' }, headline: ok ? 'Verdict: APPROVE. The errors are wrapped and the test names the id.' : 'Verdict: REQUEST CHANGES. One path still logs and returns.', rest: '- `go test ./...` ok\n- `go vet` clean', more: 2, findings: [], moves: [], at: iso(now - ago * MIN) });
const judges: Judge[] = [base.judges[0], ...Array.from({ length: N('judges', 30) }, (_, i): Judge => ({ key: `${i % 2 ? 'qc' : 'reviewer'}|service-${i}|j${i}`, cardId: i % 2 ? 'q' : 'rv', role: i % 2 ? 'qc' : 'reviewer', name: i % 2 ? `QC · service-${i}` : `Reviewer · service-${i}`, repo: `g/service-${i}`, latest: round(`v${i}`, i % 2 ? 'q' : 'rv', 10 + i * 3, i % 3 !== 2), earlier: i % 4 ? [round(`e${i}`, i % 2 ? 'q' : 'rv', 100 + i * 3, false)] : [] }))];

// The Live feed: ?events= of them, turns of a prompt, two tools, a thought, a message and a stop, across the agents.
let id = 0;
const ev = <K extends EventKind>(cardId: string, kind: K, at: number, extra: EventSeed<K> = {}): EventRead =>
  seedEvent({ id: ++id, home: 'ch', cardId, createdAt: iso(at), updatedAt: iso(at) }, kind, extra);
const events: EventRead[] = [];
const count = N('events', 5000);
const step = (2990 * MIN) / count;
for (let i = 0; events.length < count; i++) {
  const who = cards[i % cards.length].id;
  const at = now - 2995 * MIN + events.length * step;
  events.push(ev(who, 'prompt', at, { by: agent('chair'), status: 'delivered', title: 'Do the next step', text: 'Do the next step.' }));
  events.push(ev(who, 'tool', at + step, { title: 'Read', ...tool('read', 'read', 'Read'), status: 'completed', data: { input: { file_path: 'internal/errors.go' }, output: 'package errors', durationMs: 200 } }));
  events.push(ev(who, 'tool', at + 2 * step, { title: 'Bash', ...tool('execute', 'go', 'test ./...'), status: 'completed', data: { input: { command: 'go test ./...' }, output: 'ok', durationMs: 3000 } }));
  events.push(ev(who, 'thought', at + 3 * step, { text: 'The wrap goes at the boundary, not in the store.' }));
  events.push(ev(who, 'message', at + 4 * step, { text: `Turn ${i}: wrapped service-${i % 12}.` }));
  events.push(ev(who, 'stop', at + 5 * step, { ...turnEnd(5), status: 'end_turn', data: { durationMs: 5 * step } }));
}

const standing = stands('ch', 'Running', 'running', { working: true, verification: { label: 'Goal met', tone: 'done' } });
const fake = fakeServer({
  appSnapshot: () => snapshotOf({ chains: [chainRead(chain)], standings: [standing] }),
  chainGet: () => ({ chain: chainRead(chain), cards, edges: [], retrospectives: [], decisions, helps: [], verification: { ...base, judges }, datadog, landings: landings.map((l) => landingRead(l)), drafts: [], workflows: [],
    agents: cards.map((c, i) => ({ cardId: c.id, home: 'ch', label: 'Running', tone: 'running', doing: 'Working…', doingTone: 'busy', work: '', warms: true, waiting: [], children: i === 0 ? ['eng', 'rv', 'q'] : [], with: [], peers: [], branch: { agents: i === 0 ? 3 : 0, turns: 40, working: true }, asks: ['message'], stopSays: '', remembers: false, rev: 1 })),
    standing }),
  chainEvents: () => ({ events, truncated: false }),
  landingDiff: (landingId) => ({ landingId, from: 'abc', files: [{ path: 'internal/errors.go', added: 3, removed: 1, diff: '@@ -1,2 +1,4 @@\n package errors\n-// old\n+// new\n+// wrapped\n+// with the chain id\n' }] }),
  cardEvents: (cardId) => ({ events: events.filter((e) => e.event.cardId === cardId), truncated: false }),
  overturn: (decisionId, word) => {
    const d = opened.view?.decisions.find((x) => x.decision.id === decisionId)?.decision;
    if (d) fake.push('decisionAdded', decisionRead(decisionRecord({ id: `d_ov_${decisionId}`, chainId: 'ch', kind: 'overturn', by: actorOf('owner', 'owner@example.com', 'web'), text: `Overturned: ${d.text}`, why: word, overturns: decisionId, cardId: d.cardId, createdAt: iso(Date.now()) }), false));
    return {};
  },
});
openChain('ch');

const router = createRouter({ history: createMemoryHistory(), routes });
await router.push({ name: 'chain', params: { id: 'ch', tab } });
createApp({ render: () => h(TooltipProvider, { delayDuration: 300 }, () => h('div', { class: 'flex h-screen flex-col' }, [opened.view && h(ChainPage, { view: opened.view })])) })
  .use(router)
  .provide(apiKey, api)
  .mount('#app');
