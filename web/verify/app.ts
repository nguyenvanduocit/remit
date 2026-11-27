// The whole signed-in app (Shell) on made-up data, through the page's real client core: a fake server answers the calls
// and pushes changes. No server, no sign-in. Open /verify/app.html?at=<path>, e.g. ?at=/board, ?at=/chains/busy/tree,
// ?at=/playbooks (?at=/playbooks/backfill for a long one), ?at=/agents/engineer, ?at=/skills/shadcn-vue?file=references/components.md, ?at=/tools/mongo (also /tools/grafana, /tools/starrocks, /tools/coroot, /tools/cloudwatch), ?at=/board?new=1, ?at=/projects (?at=/board?project=p_storefront for the board narrowed to one; projectsSeed.ts; ?projects=0 has none). Seeded with the awkward cases: a busy chain of 20,000 events whose engineer is
// streaming, an agent asking leave and one that failed, chains waiting for the owner (a question, the Engineer
// asking leave and an item about the whole chain, which waits with the Chair), one waiting
// to check back, a paused one whose approvals are of an earlier version of its change, a queue of three behind an author chain the owner waits on (the Assistant's credential requests for sources, which only a typed value answers, wait in its Chair: ?at=/chains/author/tree?card=author-chair), finished ones and an archived one with an open MR (on no board, in no badge).
// The Assistant (?assistant=1 opens its panel, e.g. ?at=/chains/busy/tree?assistant=1): a long conversation (?talk=<turns>)
// with its answer streaming and the owner's next message waiting; ?as=ready has it idle, ?as=failed has a failed turn wait on
// the owner. Its message and a decision it caused show in the busy chain. Its Decisions panel (?assistantPanel=decisions) is
// its feed: an answer the owner overturned, one that stands, an item left to him and an older answer, two of them unseen
// (?unseen=<n>); Overturn and opening the feed answer as the server would, pushing the feed whole. What the page was asked is in window.calls.
// ?load=<ms> holds the first snapshot back that long (the loading line, before any page); /feedback is the Feedback tab (?quiet=1: none). ?health=bad shows agents that cannot run; ?fail=1 makes the server refuse a message; ?still=1 stops the streaming;
// ?left=1 adds what a deleted chain left on disk; ?quiet=1 is a first week: no analyst, no insights, few figures on /outcomes; ?title=<ms> is how long a new chain's Chair takes to title it. window.lose() plays a lost connection (the core dials again); window.push(name, arg) plays the server pushing a change, e.g. 'chainUpdated' with a chain finished.
// For measuring: ?events=<n> sizes the busy chain (default 20,000), ?chunks=<n> streams n chunks a second (default one per 700 ms),
// ?arrive=<ms> has the engineer add a new event that often. ?to=<card> has another agent of the busy chain do the writing
// and the arriving, e.g. ?to=qc, which asks leave.
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
import { fakeDrafts } from './fakeDrafts';
import { capabilities, contextOf, telemetryOf, processesOf } from './contextSeed';
import { agentStands, boardSeed, itemWait, needs, placed, stands } from './standingSeed';
import { insightsSeed, outcomesSeed } from './outcomesSeed';
import { judgmentsSeed } from './judgmentsSeed';
import { feedbackSeed } from './feedbackSeed';
import { eventsSeed, listed } from './eventsSeed';
import { verificationSeed } from './verificationSeed';
import { agentsSeed, changeItems, changesSeed, credentialItems, givenOf, skillFileSeed, skillsSeed, toolsSeed } from './librarySeed';
import { ASSISTANT, ASSISTANT_CARD, assistantCard, assistantError, assistantEvents, assistantFeedSeed, assistantStanding, assistantView } from './assistantSeed';
import type { Actor, AgentStanding, AgentTone, Ask, Card, Chain, Change, ChainStanding, ChainView, CheckBack, DecisionRead, DoingTone, Edge, EventKind, EventRead, InboxItem, Landing, LandingState, Playbook, RepoView, SubscriptionLine, ToolSource, Verification } from '../src/core/generated/types.generated';
import { accountHealth, subscriptionLine, actorOf, cardRecord, chainRead, chainRecord, changeRecord, decisionRead, decisionRecord, inboxRecord, landingRead, landingRecord, optionOf, permissionAsk, playbookRecord, repoRecord, seedEvent, seedRecord, type EventSeed } from './records';
import { signalSourcesSeed } from './signalSourcesSeed';
import { chainProjects, projectChangesSeed, projectsSeed } from './projectsSeed';
import '../src/theme.css';
import { startedOver, tool, turnEnd } from './activitySeed';

const params = new URLSearchParams(location.search);
const MIN = 60_000;
const now = Date.now();
const iso = (ms: number) => new Date(ms).toISOString();
const repo = (name: string) => repoRecord({ repo: `g/${name}`, project: `g/${name}`, path: '/', branch: 'feat/x', base: 'main' });
// What each chain lands (ADR-0114), by chain: its title and state, version 1 on the repository it works in.
const LANDS: Record<string, [repo: string, title: string, state: LandingState['kind']][]> = {
  mr: [['remit', 'fix(store): wrap store errors with the chain id', 'landed'], ['runtime', 'fix(runtime): log the wrapped store errors', 'checking']],
  watch: [['checkout', 'fix(checkout): apply the coupon before the tax', 'landed']],
  done2: [['remit', 'refactor: rename the retry fields', 'landed']],
  ok: [['sales-engine', 'perf(currency): cache the shop currency lookups', 'landed']],
  unv: [['sales-engine', 'chore: retire the legacy export endpoint', 'landed']],
  arch: [['remit', 'chore: rename the sync fields', 'ready']],
};
const landingsOf = (c: Chain): Landing[] => (LANDS[c.id] ?? []).map(([name, title, state], i) =>
  landingRecord({ id: `ln_${c.id}_${i}`, chainId: c.id, repo: `g/${name}`, version: { number: 1, title, verdicts: ['v1', 'v2'] },
    state: { kind: state } as LandingState, createdAt: iso(now - 40 * MIN),
    ...(state === 'landed' ? { commit: 'c0ffee12', push: 'c0ffee12', landedAt: iso(now - 20 * MIN), checks: { state: 'passed', failed: '' } } : { readyAt: iso(now - 10 * MIN) }) }));
// Each repository as the server lays it out: its queue being checked, then ready, then what landed; a version still judged
// is in none. No release yet.
const ORDER: Partial<Record<LandingState['kind'], number>> = { combining: 0, checking: 0, ready: 1 };
const repoViews = (): RepoView[] => [...new Set(chains.flatMap(landingsOf).map((l) => l.repo))].map((repo) => ({
  repo, releases: [],
  queues: [{ base: 'main', landings: chains.flatMap(landingsOf).filter((l) => l.repo === repo && l.state.kind !== 'judging').sort((a, b) => (ORDER[a.state.kind] ?? 3) - (ORDER[b.state.kind] ?? 3)) }],
}));

const chain = (id: string, title: string, status: Chain['status'], ageMin: number, extra: Partial<Chain> = {}): Chain => chainRecord({
  id, title, goal: `${title}: find the root cause, fix it, review it and verify.`, status, chairId: `${id}-chair`,
  workdir: `~/.remit/work/${id}`, origin: 'owner', createdAt: iso(now - ageMin * MIN), updatedAt: iso(now - Math.min(ageMin, 5) * MIN), rev: 1, ...extra,
});
// ?trace=1 joins Work, Signals and Results through the same seeded event and insight.
const traceEvent = params.has('trace') ? eventsSeed().find((e) => e.id === 'event:gitlab#7') : undefined;
if (traceEvent) {
  traceEvent.firings[0] = { ...traceEvent.firings[0], subscriptionId: 'sub_trace', title: 'Start the export investigation', action: 'start', chainId: 'ch_9f2c1e', outcome: 'Started the export investigation' };
}
const feedback = params.get('quiet') ? [] : feedbackSeed(now);
if (traceEvent && feedback.length) feedback[0] = { ...feedback[0], home: 'ch_9f2c1e', cardId: 'ch_9f2c1e-chair' };
const chains: Chain[] = [
  ...(traceEvent ? [chain('ch_9f2c1e', 'Verify the export with a real file', 'done', 600, { eventId: traceEvent.id, origin: 'event' })] : []),
  chain('busy', 'Backfill missing zone groups', 'running', 600),
  chain('mr', 'Wrap store errors with the chain id', 'running', 300, { repos: [repo('remit'), repo('runtime')], summary: 'Two changes: the store wraps errors, the runtime logs them.' }),
  chain('ask', 'Decide the retention of old events', 'running', 90),
  chain('wait', 'Watch the nightly import', 'waiting', 240),
  chain('paused', 'Split the orchestrator package', 'paused', 400),
  // Merged, and its Chair watches the signal that says whether the change did what the goal asked (ADR-0013).
  chain('watch', 'Checkout error rate stays low after the coupon change', 'waiting', 500, { repos: [repo('checkout')] }),
  chain('q2', 'Second in the queue', 'queued', 30, { origin: 'routine' }),
  chain('q1', 'First in the queue', 'queued', 45, { origin: 'proposed' }),
  chain('q3', 'Nightly sweep of stale branches', 'queued', 10, { origin: 'routine' }),
  chain('done1', 'Fix the empty body in the overview handler', 'done', 2000, { summary: '## Fixed\nThe handler returned `{}` when the cache was cold.' }),
  chain('done2', 'Rename the retry fields', 'done', 3000, { repos: [repo('remit')] }),
  chain('ok', 'Cache the shop currency lookups', 'done', 4000, { repos: [repo('sales-engine')], summary: 'Landed, live, and the checkout latency fell from 410 ms to 180 ms over the window: every key result is met.' }),
  chain('unv', 'Retire the legacy export endpoint', 'done', 4500, { repos: [repo('sales-engine')], summary: 'Landed. The window ended with no reading from the access log, so the key result that nobody calls the endpoint is unverified.' }),
  chain('arch', 'An archived chain whose change is still in the queue', 'done', 5000, { archivedAt: iso(now - 1000 * MIN), repos: [repo('remit')] }),
  chain('author', 'Playbook · add a security review step', 'running', 3, { kind: 'author', capabilities: ['agents', 'history', 'library', 'playbooks'] }),
  chain('author2', 'Playbook · put customer bugs first', 'queued', 2, { kind: 'author', capabilities: ['agents', 'history', 'library', 'playbooks'] }),
];
// When the waiting chains' Chairs come back, as the server works it out from their wake subscriptions: the watching
// one's watch ends first.
const checkBacks: Record<string, CheckBack> = {
  wait: { at: iso(now + 50 * MIN), why: 'the import runs at 02:00: check that !191 merged and prod-build-deploy succeeded, then compare the code-17 count, shop 4649943153\'s call rate and SyncAdDetailsQueue depth against today\'s baseline', setAt: iso(now - 10 * MIN) },
  watch: { at: iso(now + 3 * 60 * MIN), why: 'The coupon change leaves the checkout error rate under 2% for a day.', setAt: iso(now - 20 * MIN),
    watch: { key: 'chain:watch.sensor.checkout-error-rate', condition: 'value.rate >= 0.02', until: iso(now + 3 * 60 * MIN), why: 'The coupon change leaves the checkout error rate under 2% for a day.' } },
};
const readOf = (c: Chain) => chainRead(c, checkBacks[c.id]);

const card = (chainId: string, id: string, agent: string, name: string, extra: Partial<Card> = {}): Card => cardRecord({
  id, home: chainId, agent, name, origin: agent === 'chair' ? 'chair' : 'hired', model: agent === 'chair' ? 'opus' : 'sonnet', mode: 'auto', cwd: `~/.remit/work/${chainId}`,
  status: 'idle', plan: [], ctxUsed: 42_000, ctxSize: 200_000, turns: 3, queued: 0, sessionId: `s-${id}`, createdAt: iso(now - 600 * MIN), updatedAt: iso(now - 2 * MIN), rev: 1, ...extra,
});
// The cards and decisions of chains made on this page (chainCreate), as the server adds them.
const madeCards = new Map<string, Card[]>();
const madeDecisions = new Map<string, DecisionRead[]>();
function cardsOf(c: Chain): Card[] {
  if (madeCards.has(c.id)) return madeCards.get(c.id)!;
  const chair = card(c.id, c.chairId, 'chair', 'Chair', { status: c.status === 'running' ? 'running' : 'idle', plan: [
    { content: 'Find where zones lose their group', priority: 'high', status: 'completed' },
    { content: 'Backfill the missing groups', priority: 'high', status: 'in_progress' },
    { content: 'Have QC verify the counts', priority: 'medium', status: 'pending' },
  ] });
  const eng = card(c.id, `${c.id}-eng`, 'engineer', 'Engineer', { createdAt: iso(now - 80 * MIN), status: c.status === 'done' ? 'stopped' : c.id === 'ask' ? 'permission' : 'idle', output: 'done' });
  // Remit's checks of a finish: goal judges, cards outside the team. The finished chain was refused once, then passed;
  // the Playbook author chain was refused and its Chair is back at work.
  const judge = (n: number, ago: number) => card(c.id, `${c.id}-judge${n}`, 'judge', `Goal judge · check ${n}`, { origin: 'judge', createdAt: iso(now - ago * MIN), status: 'stopped', turns: 1, sessionId: `s-judge${n}`,
    why: "Checks the Chair's result against the goal before the chain finishes." });
  if (c.id === 'done1') return [chair, eng, judge(1, 60), judge(2, 30)];
  if (c.id === 'author') return [chair, eng, judge(1, 2)];
  if (c.id !== 'busy') return [chair, eng];
  return [
    chair,
    card(c.id, 'analyst', 'research', 'Analyst', { createdAt: iso(now - 590 * MIN), status: 'stopped', output: 'Report: 53 zones lack a group.' }),
    card(c.id, 'eng', 'engineer', 'Engineer — backfill zones', { createdAt: iso(now - 500 * MIN), status: 'running', ctxUsed: 172_000, why: 'The analyst found 53 zones without a group; someone has to backfill them.', brief: 'Backfill the groups.', criteria: ['every zone has a group', 'a test proves it'] }),
    card(c.id, 'eng2', 'engineer', 'Engineer — migration', { createdAt: iso(now - 400 * MIN), status: 'failed', error: 'the session closed: out of context' }),
    card(c.id, 'qc', 'qc', 'QC', { createdAt: iso(now - 300 * MIN), status: 'permission' }),
    card(c.id, 'rev', 'reviewer', 'Reviewer', { createdAt: iso(now - 200 * MIN), awaitingReview: true }),
  ];
}
// How each chain's result has been judged, as the server reads its verdicts: done1 as the seed's chain was (a reviewer's
// second round, a refused finish then a passed one, by its goal judges judge1 and judge2), mr in review, and the Playbook
// author chain refused once (its judge1), its Chair back at work.
// done1 was judged as the seed's chain was (a reviewer's second round, a refused finish then a passed one); mr is in review;
// author's first finish was refused; paused was approved, then an engineer committed on top.
const verificationOf = (c: Chain): Verification =>
  verificationSeed(c.id, c.id === 'mr' ? 'security' : c.id === 'done1' || c.id === 'ok' || c.id === 'unv' || c.id === 'ch_9f2c1e' ? 'judged' : c.id === 'author' ? 'refused' : c.id === 'paused' ? 'stale' : 'none');
const edgesOf = (c: Chain): Edge[] => (c.id === 'busy' ? [{ id: 'e1', chainId: 'busy', from: 'eng', to: 'rev', kind: 'handoff', label: 'reviews', createdAt: iso(now - 200 * MIN) } as Edge, { id: 'e2', chainId: 'busy', from: 'eng', to: 'qc', kind: 'handoff', label: 'checks', createdAt: iso(now - 300 * MIN) } as Edge] : []);
// Who did each: the Chair, Remit, the owner, and in the busy chain the Assistant for him.
const BY: Actor[] = [actorOf('agent'), actorOf('remit'), actorOf('agent'), actorOf('owner', 'owner@example.com', 'web')];
const decisionsOf = (c: Chain): DecisionRead[] =>
  madeDecisions.get(c.id) ?? [
    ...(['hire', 'route', 'accept', 'rework', 'landing', 'ask', 'finish'] as const).map((kind, i) => decisionRead(decisionRecord({ id: `${c.id}-d${i}`, chainId: c.id, kind, by: BY[i % BY.length].kind === 'agent' ? actorOf('agent', c.chairId) : BY[i % BY.length], text: `Step ${i + 1} of ${c.title}`, why: i % 2 ? 'Because the report said so.' : '', createdAt: iso(now - (70 - i * 10) * MIN) }), i === 4 || i === 6)),
    ...(c.id === 'watch' ? [decisionRead(decisionRecord({ id: 'watch-live', chainId: c.id, kind: 'live', by: actorOf('assistant', ASSISTANT), text: 'Marked !31 live: the deploy finished at 14:05', why: 'The owner said so from the board: "the coupon change is live".', createdAt: iso(now - 25 * MIN) }), true)] : []),
    // Remit released what the chain landed and it baked healthy (ADR-0116): the builder's chain reads both, as outcomes.
    ...(c.id === 'ok' ? [
      decisionRead(decisionRecord({ id: 'rel1', chainId: c.id, kind: 'release', by: actorOf('remit', 'release'), text: 'g/checkout was released as v0.42.0 at https://shop.example: it carries Cache the shop currency lookups. It bakes until 14:05 under chain:ok.sensor.checkout-errors: a breach takes it back, and you are told.', why: 'release rel_42', createdAt: iso(now - 15 * MIN) }), true),
      decisionRead(decisionRecord({ id: 'rel2', chainId: c.id, kind: 'release', by: actorOf('remit', 'release'), text: 'g/checkout is live in v0.42.0.', why: 'release rel_42', createdAt: iso(now - 8 * MIN) }), true),
    ] : []),
    ...(c.id === 'busy' ? [decisionRead(decisionRecord({ id: 'busy-das', chainId: c.id, kind: 'pause', by: actorOf('assistant', ASSISTANT), text: 'Paused the QC re-check until the backfill is done', why: 'The owner asked from the board: "hold QC until the zones are in".', createdAt: iso(now - 5 * MIN) }), true)] : []),
  ];

// The busy chain's history: 20,000 events, turns of a prompt, a few tools, a message and a stop, across its agents.
let nextId = 0;
const OWNER = actorOf('owner', 'owner@example.com', 'web');
const REMIT = actorOf('remit');
const agent = (cardId: string): Actor => actorOf('agent', cardId);
const ev = <K extends EventKind>(chainId: string, cardId: string, kind: K, at: number, extra: EventSeed<K> = {}): EventRead =>
  seedEvent({ id: ++nextId, home: chainId, cardId, createdAt: iso(at), updatedAt: iso(at) }, kind, extra);
function historyOf(c: Chain): EventRead[] {
  const out: EventRead[] = [];
  const agents = cardsOf(c).map((k) => k.id);
  const count = c.id === 'busy' ? Number(params.get('events') ?? 20_000) : 60;
  const start = new Date(c.createdAt).getTime();
  const step = (now - 3 * MIN - start) / count;
  for (let i = 0; out.length < count; i++) {
    const who = agents[i % agents.length];
    const at = start + out.length * step;
    out.push(ev(c.id, who, 'prompt', at, { by: who === c.chairId ? REMIT : agent(c.chairId), status: 'delivered', title: who === c.chairId ? 'Read the report from Engineer' : 'Do the next step', text: who === c.chairId ? '[report from Engineer · card eng · stop end_turn]' : 'You are an agent of this chain. Do the next step.' }));
    out.push(ev(c.id, who, 'tool', at + step, { title: 'Read', ...tool('read', 'read', 'Read'), status: 'completed', data: { input: { file_path: 'internal/orch/zones.go' }, output: 'package orch', durationMs: 300 } }));
    out.push(ev(c.id, who, 'tool', at + 2 * step, { title: 'Bash', ...tool('execute', 'go', 'test ./internal/orch/'), status: 'completed', data: { input: { command: 'go test ./internal/orch/' }, output: 'ok', durationMs: 4000 } }));
    out.push(ev(c.id, who, 'thought', at + 3 * step, { text: 'The groups are lost when a zone is renamed.' }));
    out.push(ev(c.id, who, 'message', at + 4 * step, { text: `Turn ${i}: backfilled ${i % 53} zones.` }));
    out.push(ev(c.id, who, 'stop', at + 5 * step, { ...turnEnd(5), status: 'end_turn', data: { durationMs: 5 * step } }));
  }
  // The Assistant passed on the owner's words to the Chair.
  if (c.id === 'busy') out.push(ev(c.id, 'busy-chair', 'prompt', now - 2 * MIN - 10_000, { by: actorOf('assistant', ASSISTANT), status: 'delivered', title: 'From the Assistant', text: 'Have QC re-check the last 3 zones.' }));
  // The engineer is writing right now.
  if (c.id === 'busy') out.push(ev(c.id, 'eng', 'prompt', now - 2 * MIN, { by: agent('busy-chair'), status: 'delivered', title: 'Rework after feedback', text: '[feedback from the Chair · rework]\nfix the last 3 zones' }), ev(c.id, 'eng', 'message', now - MIN, { text: 'Backfilling the last zones' }));
  return out;
}
const histories = new Map(chains.map((c) => [c.id, historyOf(c)]));
const writer = params.get('to') ?? 'eng';
if (writer !== 'eng') histories.get('busy')!.push(ev('busy', writer, 'message', now - MIN / 2, { text: 'Counting the zones on staging' }));

const inbox: InboxItem[] = [
  ...(params.get('left') ? [inboxRecord({ id: 'i9', home: 'ch_deleted', cardId: '', kind: 'error', title: 'Files of deleted chain ch_deleted are left', detail: 'worktree of g/svc kept: /work/ch_deleted/svc is on branch main, not the chain\'s\n\nRemove them by hand, then dismiss this.', options: [optionOf('dismiss', 'Dismiss')], status: 'open', createdAt: iso(now - 5 * MIN), rev: 1 })] : []),
  inboxRecord({ id: 'i1', home: 'ask', cardId: 'ask-chair', kind: 'question', title: 'How long should old events be kept?', detail: 'The table holds **4 GB**. Keep 30 days, 90 days, or everything?', options: [optionOf('30', '30 days'), optionOf('90', '90 days'), optionOf('all', 'Keep everything')], status: 'open', createdAt: iso(now - 30 * MIN), rev: 1 }),
  inboxRecord({ id: 'i2', home: 'busy', cardId: 'qc', kind: 'permission', title: 'Run psql against staging', detail: 'psql -h staging -c "select count(*) from zones"', permission: permissionAsk({ action: 'Run psql against staging', reason: 'It reaches another machine, so it runs only if it is allowed.', traits: [{ kind: 'network', says: 'network' }], risk: { kind: 'medium', says: 'Medium risk' }, tool: 'Bash', command: 'psql -h staging -c "select count(*) from zones"' }), options: [optionOf('a', 'Allow', 'allow_once'), optionOf('r', 'Reject', 'reject_once')], status: 'open', createdAt: iso(now - 4 * MIN), rev: 1 }),
  inboxRecord({ id: 'i3', home: 'busy', cardId: 'eng2', kind: 'error', title: 'Engineer — migration stopped', detail: 'the session closed: out of context', options: [optionOf('retry', 'Retry'), optionOf('stop', 'Leave it')], status: 'open', createdAt: iso(now - 9 * MIN), rev: 1 }),
  inboxRecord({ id: 'i5', home: 'ask', cardId: 'ask-eng', kind: 'permission', title: 'Delete events older than 90 days on staging', detail: 'psql -h staging -c "delete from events where created_at < now() - interval \'90 days\'"', permission: permissionAsk({ action: 'Delete events older than 90 days on staging', reason: 'It removes what cannot be brought back and reaches another machine, so it runs only if it is allowed.', traits: [{ kind: 'irreversible', says: 'cannot be undone' }, { kind: 'network', says: 'network' }], risk: { kind: 'high', says: 'High risk' }, tool: 'Bash', command: 'psql -h staging -c "delete from events where created_at < now() - interval \'90 days\'"' }), options: [optionOf('a', 'Allow', 'allow_once'), optionOf('r', 'Reject', 'reject_once')], status: 'open', createdAt: iso(now - 6 * MIN), rev: 1 }),
  // About the whole chain: it names no agent, so it waits with the Chair.
  inboxRecord({ id: 'i6', home: 'ask', cardId: '', kind: 'error', title: 'Could not remove an old worktree', detail: 'rm ~/.remit/work/ask/old: permission denied', options: [optionOf('retry', 'Retry')], status: 'open', createdAt: iso(now - 12 * MIN), rev: 1 }),
  ...(params.get('as') === 'failed' ? [assistantError(now)] : []),
  // The Assistant asked for the values of three source variables, which the owner has not given yet: they wait in that chain's Chair.
  ...credentialItems(now),
  // And two writes that move a rail, which the owner has to let through.
  ...changeItems(now),
  inboxRecord({ id: 'i4', home: 'arch', cardId: 'arch-chair', kind: 'question', title: 'A question on an archived chain', detail: 'It is on no board.', options: [], status: 'open', createdAt: iso(now - 900 * MIN), rev: 1 }),
];
// ?key=1: what waits is only a credential (ADR-0087), so Needs you, the palette and the title count hold that alone.
if (params.get('key')) inbox.splice(0, inbox.length, ...inbox.filter((i) => i.kind === 'credential').slice(0, 1));
const playbook = (id: string, name: string, extra: Partial<Playbook> = {}): Playbook => playbookRecord({
  id, name, description: `${name}: how the Chair usually goes about it.`, group: 'engineering', brief: 'Hire freely. Ask the owner only what cannot be found.', rev: 1,
  seeds: [seedRecord({ agent: 'research', name: 'Analyst', brief: 'Find where it breaks.' }), seedRecord({ agent: 'engineer', name: 'Engineer', brief: 'Fix it, with a test first.' }), seedRecord({ agent: 'qc', name: 'QC', brief: 'Verify on the real system.' })], ...extra,
});
let sources = toolsSeed();
// What was written to the records above, by `kind/name`; a write the owner lets through adds to it.
const changeHistory = { ...changesSeed(now), ...projectChangesSeed(now) };
// The awkward one, as the real backfill playbook is: a brief of one long paragraph, long seed briefs, cards that hire
// plain agents (research, spike) beside the library's, cards named after their agent.
const backfill = playbook('backfill', 'Backfill / migration', {
  description: 'Prepare a backfill or migration for the owner to run: dry-run, idempotent, with rollback.',
  brief: 'Prepare only: never run a migration or a backfill against a shared store, and never deploy; the owner runs it. The deliverable is a landed change with the script and its runbook. Run a Spike before the script is written: no engineer is hired until the spike has checked the real schema and data (read-only) and the riskiest assumption behind the approach (the record set, the shape of the documents or rows, the volume and how long a batch takes, the logic that recomputes the value) and come back with go or no-go. If the spike says no-go, change the approach before building. When the migration serves something new (a new field, a new behaviour, new data the product has not had) rather than repairing existing data, hire a PO before the script to fix what the new data must mean and its acceptance criteria; open product choices get the most conservative reading, listed as assumptions in the MR. The script touches an exact, stated set of records, and a query counts them. It does nothing by default: a dry-run prints what it would change, and writing takes an explicit flag. It is idempotent (a second run changes nothing), works in batches with a resume point, and has a rollback or says plainly why it has none. The queries that prove it worked come with it. Test it on fixtures or a local copy, never on live data. The cause of the bad data is fixed by its own MR on the write path, not by the backfill, so the problem does not come back. Say what is irreversible and what the owner must check before running.',
  builtin: true, edited: true,
  seeds: [
    seedRecord({ agent: 'research', name: 'Scope', brief: 'Find which records are wrong or missing and why: the query that counts them, a few concrete examples by id, and the cause with file:line. The cause is fixed by a separate change.' }),
    seedRecord({ agent: 'spike', name: 'Spike', brief: 'Before the script is written, test the approach against reality, read-only and time-boxed. State the riskiest assumptions first (record set, document/row shape, volume, the logic that recomputes the value, indexes the batch query needs), then check each one on the real schema and data with read-only queries (staging, or production reads through Bytebase) and a throwaway prototype of the core logic run on a sample of real ids. Never write to a shared store; prototype code is not merged. Deliver: each assumption with what you ran, what you saw (counts, ids, timings) and whether it held; the edge cases found in the data; go or no-go for the approach with a confidence label.' }),
    seedRecord({ agent: 'engineer', name: 'Script', brief: 'Write the script on a branch with tests on fixtures: dry-run by default, writing only behind an explicit flag, idempotent, batched with a resume point. Cover the edge cases the Spike found. Report how to run it and what it prints.' }),
    seedRecord({ agent: 'reviewer', name: 'Reviewer', brief: 'Review it as code that will run once on production data: wrong record set, writes that are not idempotent, unbounded batches, missing rollback; findings with file:line and severity; verdict.' }),
    seedRecord({ agent: 'qc', name: 'QC', brief: 'Run the dry-run, then the real run twice, on fixtures or a local copy; report the counts before and after, that the second run changed nothing, and that the proving queries catch a seeded bad record.' }),
  ],
});
const playbooks = [playbook('bugfix', 'Fix a bug'), playbook('feature', 'Build a feature'), playbook('playbook-author', 'Playbook author', { seeds: [], builtin: true }), backfill];

// What the owner can ask of a chain and who it came from, as the server would say it (made up after orch's chainAsksOf
// and originLabel): an archived chain is unarchived, a finished one with a change still in its queue is not deleted.
function offered(c?: Chain): Pick<ChainStanding, 'asks' | 'from'> {
  if (!c) return { asks: [], from: 'you' };
  const from = { owner: 'you', proposed: "a chain's proposal", routine: 'a routine', event: 'an event', client: 'an outside agent', verification: "a judge's verification", replay: 'a replay' }[c.origin ?? 'owner'];
  if (c.status !== 'done') return { asks: [...(c.status === 'paused' ? ['resume'] : c.status === 'queued' ? [] : ['pause']), 'finish'] as ChainStanding['asks'], from };
  const open = landingsOf(c).some((l) => ['judging', 'ready', 'combining', 'checking'].includes(l.state.kind));
  return { asks: [c.archivedAt ? 'unarchive' : 'archive', ...(open ? [] : ['delete'])] as ChainStanding['asks'], from };
}
// What waits for the owner, as the server lists it (made up after orch waiting.rs, ADR-0078): every open inbox item, the
// archived chain's question too, oldest first; who each is about, in words. The newest arrived since he last looked.
const WHO: Record<string, string> = { qc: 'QC', eng2: 'Engineer', 'ask-chair': 'Chair', 'ask-eng': 'Engineer', 'author-chair': 'Chair', 'arch-chair': 'Chair', assistant: 'Assistant', [ASSISTANT_CARD]: 'Assistant' };
const waiting = [
  ...inbox.map((i) => itemWait(i, WHO[i.cardId] ?? 'Remit', chains.some((c) => c.id === i.home))).sort((a, b) => a.since.localeCompare(b.since)),
].map((w, i, all) => (i === all.length - 1 ? { ...w, unseen: true } : w));
// Where everything stands, as the server would say it for these chains (made up, see standingSeed.ts): the column that needs
// the owner holds its chains by their first wait, in the list's order.
const left = [...(params.get('left') ? ['i9'] : []), ...(params.get('key') ? inbox.map((i) => i.id) : [])];
const needing = [...new Set(waiting.flatMap((w) => (w.chainId && w.chainId !== 'arch' ? [w.chainId] : [])))];
let board = boardSeed({ needs: needing, running: ['mr', 'paused'], waiting: ['wait'], watching: ['watch'], queued: ['author2', 'q1', 'q2', 'q3'], finished: ['ok', 'unv', 'done1', 'done2', ...(traceEvent ? ['ch_9f2c1e'] : [])] }, left, 1, waiting);
const standings: ChainStanding[] = [
  ...(traceEvent ? [stands('ch_9f2c1e', 'Done', 'done', { verification: { label: 'Goal met', tone: 'done' } })] : []),
  ...['busy', 'ask', 'author'].map((id) => needs(id, waiting)),
  // Its change touches a rail: the reviewer and the QC passed it, the security reviewer has not yet (ADR-0087).
  stands('mr', 'Running', 'running', { working: true, verification: { label: 'Awaiting security', tone: 'needs' } }),
  stands('paused', 'Paused', 'paused', { verification: { label: 'Changed since approval', tone: 'needs' } }),
  stands('wait', 'Waiting', 'waiting', { checkBackAt: iso(now + 50 * MIN), why: 'the import runs at 02:00: check that !191 merged and prod-build-deploy succeeded, then compare the code-17 count, shop 4649943153\'s call rate and SyncAdDetailsQueue depth against today\'s baseline' }),
  ...['author2', 'q1', 'q2'].map((id, i) => stands(id, 'Waiting for its turn', 'queued', { place: i + 1 })),
  stands('q3', 'Held for your review', 'queued', { place: 4, why: '6 approved changes wait in the merge queue, and at most 5 may before what Remit starts on its own waits: it starts once one lands' }),
  stands('watch', 'Watching', 'watching', { checkBackAt: iso(now + 3 * 60 * MIN), why: 'The coupon change leaves the checkout error rate under 2% for a day.', verification: { label: 'Goal met', tone: 'done' } }),
  stands('ok', 'Done', 'done', { verification: { label: 'Goal met', tone: 'done' }, unseen: true }),
  stands('unv', 'Not done · not proven: nobody calls the endpoint', 'notdone', { verification: { label: 'Goal met', tone: 'done' } }),
  ...['done1', 'done2', 'arch'].map((id) => stands(id, id === 'done1' ? 'Done' : 'Not done · ended by you before it was checked', id === 'done1' ? 'done' : 'notdone', { verification: id === 'done1' ? { label: 'Goal met', tone: 'done' } : undefined })),
].map((st) => ({ ...st, ...offered(chains.find((c) => c.id === st.chainId)), project: chainProjects[st.chainId] }));
const ITEMS: Record<string, string[]> = { qc: ['i2'], eng2: ['i3'], 'ask-chair': ['i1', 'i6'], 'ask-eng': ['i5'], 'author-chair': ['i7', 'i8', 'i12', 'i10'] };
function agentsOf(c: Chain): AgentStanding[] {
  return placed(c.chairId, cardsOf(c), standsOf(c), c.id === 'busy' ? { rev: 'eng', qc: 'eng' } : {});
}
// What the owner can ask of an agent, as the server would say it (made up after orch's asksOf): a judge that is over can be asked (its session resumes), never restarted.
function asksOf(c: Chain, k: Card): Ask[] {
  if (!['stopped', 'failed'].includes(k.status) && c.status !== 'done') {
    const busy = k.status === 'running' || k.status === 'permission';
    return ['message', ...(busy ? (['steer', 'interrupt'] as const) : []), ...(k.origin === 'chair' ? [] : (['stop'] as const))];
  }
  if (k.sessionId) return ['resume'];
  return c.status === 'done' || k.origin === 'judge' ? [] : ['restart'];
}
function standsOf(c: Chain): AgentStanding[] {
  const judges = verificationOf(c).judges;
  return cardsOf(c).map((k) => {
    const waiting = ITEMS[k.id] ?? [];
    const own = (label: string, tone: AgentTone, doing: string, doingTone: DoingTone) => agentStands(c.id, k.id, label, tone, doing, doingTone, waiting, { asks: asksOf(c, k) });
    if (k.id === 'qc') return own('Needs you', 'needs', 'Approve: Run psql against staging', 'needs');
    if (k.id === 'eng2') return own('Needs you', 'failed', 'Failed: Engineer — migration stopped', 'error');
    if (k.id === 'ask-chair') return own('Needs you', 'failed', 'Failed: Could not remove an old worktree', 'error');
    if (k.id === 'ask-eng') return own('Needs you', 'needs', 'Approve: Delete events older than 90 days on staging', 'needs');
    if (k.agent === 'judge' && k.status === 'stopped') {
      const j = judges.find((x) => x.cardId === k.id);
      return own(c.status === 'done' ? 'Done' : 'Stopped', 'done', j?.latest.check.label ?? 'Gave no verdict', 'done');
    }
    if (k.origin === 'judge' && k.status === 'stopped') return own('Done', 'done', 'Chose the playbook and the title', 'done');
    if (k.origin === 'judge') return own('Running', 'running', 'Thinking…', 'busy');
    if (k.status === 'running') return own('Running', 'running', k.origin === 'chair' ? 'Reading what Engineer sent…' : 'Replying…', 'busy');
    if (k.awaitingReview) return own('Awaiting acceptance', 'waiting', 'Reported · waiting for the Chair to accept', 'waiting');
    if (c.status === 'paused') return own('Idle', 'idle', 'Chain is paused', 'waiting');
    if (c.status === 'waiting' && k.origin === 'chair') return own('Idle', 'idle', 'Chain is waiting to check back', 'waiting');
    if (k.status === 'stopped') return own(c.status === 'done' ? 'Done' : 'Stopped', 'done', c.status === 'done' ? 'Done · message to ask more' : 'Stopped · message to resume', 'done');
    if (k.origin === 'chair') return own('Idle', 'idle', c.status === 'done' ? 'Chain is done' : 'Idle · waiting for your direction', c.status === 'done' ? 'done' : 'idle');
    return own('Idle', 'idle', 'Turn done · accepted by the Chair', 'done');
  });
}
// The Assistant, where it stands, and its conversation.
const asState = (params.get('as') ?? 'working') as 'working' | 'ready' | 'failed';
let asCard = assistantCard(now, asState === 'working' ? {} : { status: asState === 'failed' ? 'failed' : 'idle', queued: 0, activity: '', error: asState === 'failed' ? 'the provider refused the request: overloaded' : '' });
let asAgent = assistantStanding(asState, asState === 'failed' ? ['i_as'] : []);
const asEvents = assistantEvents(now, Number(params.get('talk') ?? 120));
if (asState !== 'working') asEvents.splice(asEvents.findIndex((e) => e.event.status === 'queued'), 1);
const calls: unknown[] = [];
Object.assign(window, { calls });
const view = (id: string): ChainView => {
  const c = chains.find((x) => x.id === id);
  if (!c) throw new Error(`chain ${id} not found`);
  return { chain: readOf(c), cards: cardsOf(c), edges: edgesOf(c), decisions: decisionsOf(c), helps: [], verification: verificationOf(c), datadog: [], landings: landingsOf(c).map((l) => landingRead(l)), drafts: drafts.live(id), workflows: [], agents: agentsOf(c),
    standing: standings.find((st) => st.chainId === id) ?? stands(id, 'Done', 'done'), retrospectives: [] };
};
// The owner's drafts (a message half written in a composer) outlive a reload of the page and show in its other tabs.
const drafts = fakeDrafts('app', (name, arg) => fake.push(name as never, arg as never));
// What the chains wait for, as the Signals board lists it.
const appSubscriptions: SubscriptionLine[] = [
  ...(traceEvent ? [subscriptionLine({ kind: 'event', id: 'sub_trace', title: 'Start the export investigation', key: 'event:gitlab', source: 'event', what: 'Investigate the failing export', state: 'waiting', group: 'coming', control: '' })] : []),
  subscriptionLine({ kind: 'check_back', id: 'sub_wait', title: 'Import the orders', chainId: 'wait', source: 'clock', at: iso(now + 50 * MIN), what: 'the import runs at 02:00', state: 'waiting', group: 'coming', control: '' }),
  subscriptionLine({ kind: 'watch', id: 'sub_watch_watch_a1', title: 'Coupon change', chainId: 'watch', source: 'sensor', key: 'chain:watch.sensor.checkout-error-rate', condition: 'value.rate >= 0.02', at: iso(now + 3 * 60 * MIN), what: 'The coupon change leaves the checkout error rate under 2% for a day.', state: 'waiting', group: 'coming', control: '' }),
  subscriptionLine({ kind: 'watch', id: 'sub_watch_watch_b2', title: 'Coupon change', chainId: 'watch', source: 'landing', key: 'chain:watch.landing.g/checkout', at: iso(now + 26 * 60 * MIN), what: 'The change stays landed: nothing takes it back.', state: 'held', group: 'attention', control: '', held: 'its sensor has failed 3 reads in a row (last error: connection refused); the deadline still wakes the Chair' }),
  subscriptionLine({ kind: 'routine', id: 'rt_audit', title: 'Dependency audit', source: 'clock', at: iso(now + 90 * MIN), what: 'Audit the dependencies of the API · cron 0 9 * * 1-5', state: 'waiting', group: 'coming', control: '' }),
];
// The Assistant's feed, as the server holds it; a seen mark of the feed and overturn change it and push it whole, as the server
// does. A seen mark of a chain is recorded in window.calls.
const feed = assistantFeedSeed(now, Number(params.get('unseen') ?? 2));
const pushFeed = () => fake.push('assistantFeedUpdated', structuredClone(feed));
const fake = fakeServer({
  seenMark: (subject) => {
    calls.push(['seenMark', subject]);
    if (subject.kind === 'assistant_feed') (feed.unseen = 0), pushFeed();
    return {};
  },
  overturn: (decisionId, word) => {
    calls.push(['overturn', decisionId, word]);
    if (params.get('fail')) throw new Error('the server refused it');
    const l = feed.lines.find((x) => x.decision.id === decisionId);
    if (!l || l.overturn || !l.answered) throw new Error(`decision ${decisionId} is no answer the Assistant gave for you`);
    l.overturn = decisionRecord({ id: 'd_ov_' + decisionId, chainId: l.decision.chainId, kind: 'overturn', by: OWNER, text: 'Overturned: ' + l.decision.text.slice('Decided for you: '.length), why: word.trim(), item: l.decision.item, overturns: decisionId, createdAt: new Date().toISOString() });
    pushFeed();
    return {};
  },
  draftSave: drafts.draftSave,
  draftDelete: drafts.draftDelete,
  appSnapshot: async () => (await new Promise((done) => setTimeout(done, Number(params.get('load') ?? 0))), snapshotOf({
    objectives: [], projects: params.get('projects') === '0' ? [] : projectsSeed(now), employees: [], mentors: [], mentorStandings: [],
    // The archived chains are left out, as the server leaves them: chainsArchived reads them.
    chains: chains.filter((c) => !c.archivedAt).map(readOf), inbox, playbooks, capabilities, board, standings: standings.filter((st) => !chains.find((c) => c.id === st.chainId)?.archivedAt),
    agents: agentsSeed(), skills: skillsSeed(now), sources, events: traceEvent ? [listed(traceEvent)] : [], eventSignals: [], sensors: [], clientKeys: [], extensions: [], feedback,
    health: {
      ...(params.get('health') === 'bad'
        ? { ok: false, error: 'default: command not found', accounts: [accountHealth({ id: 'default', kind: 'machine', ok: false, error: 'command not found' })] }
        : { ok: true, error: '', accounts: [accountHealth({ id: 'default', kind: 'machine', ok: true })] }),
      checkedAt: iso(now),
      // ?unreadable=3: records a newer build wrote, which this one leaves out (ADR-0107)
      unreadable: Number(params.get('unreadable') ?? 0),
      rev: 1,
    },
    workload: { running: 2, capacity: 3, pressure: '', queued: 3, waiting: 2, nextCheckBack: iso(now + 50 * MIN), rev: 1 },
    signals: { rev: 1, held: 1, signals: [], signalsMore: 0, subscriptions: appSubscriptions, sources: signalSourcesSeed([], appSubscriptions) },
    insights: params.get('quiet') ? [] : insightsSeed(),
    assistant: { ...assistantView(now, asCard, asAgent), drafts: drafts.live(ASSISTANT) },
    assistantFeed: structuredClone(feed),
  })),
  chainsArchived: async () => ({ chains: chains.filter((c) => c.archivedAt).map(readOf), standings: standings.filter((st) => chains.find((c) => c.id === st.chainId)?.archivedAt), next: '' }),
  changesGet: async (kind, name) => changeHistory[`${kind}/${name}`] ?? [],
  skillFile: skillFileSeed(now),
  outcomesGet: async (last, window) => outcomesSeed(!!params.get('quiet'), last, window),
  judgmentsGet: async () => judgmentsSeed(!!params.get('quiet'), now),
  analystRun: async () => chains[0],
  chainGet: async (id) => view(id),
  eventGet: async (id) => {
    if (traceEvent?.id === id) return traceEvent;
    throw new Error(`Event ${id} is missing from the fixture`);
  },
  // The chain's newest events, as many as the server answers (`CHAIN_EVENTS_SHOWN`, crates/wire/src/app.rs).
  chainEvents: async (id) => {
    const all = histories.get(id) ?? [];
    return { events: all.slice(-600), truncated: all.length > 600 || !!params.get('earlier') };
  },
  // A card's page holds its newest events, as many as the server's page (`CARD_EVENTS_PAGE`), or with ?earlier=<n> its
  // newest n; the page before the oldest held is read on demand.
  cardEvents: async (cardId, before) => {
    const all = cardId === ASSISTANT_CARD ? asEvents : [...histories.values()].flat().filter((e) => e.event.cardId === cardId);
    const older = before ? all.filter((e) => e.event.id < before) : all;
    // A page begins at the newest Start over in it, as the server's does.
    let since = older.length - 1;
    while (since >= 0 && older[since].activity?.type !== 'started_over') since--;
    const page = since >= 0 ? older.slice(since) : older;
    const n = Number(params.get('earlier') ?? 0) || 1000;
    return { events: page.slice(-n), truncated: since >= 0 || page.length > n };
  },
  // The owner's message to the Assistant, with where he is: kept as the server keeps it, the place named in its words.
  assistantSetRuntime: async (model, effort) => {
    calls.push({ assistantSetRuntime: { model, effort } });
    asCard = { ...asCard, model: model ?? asCard.model, effort: effort ?? asCard.effort, rev: asCard.rev + 1 };
    fake.push('cardUpdated', asCard);
    return {};
  },
  assistantPrompt: async (draftId, where, delivery) => {
    calls.push({ assistantPrompt: { draftId, where, delivery } });
    const d = drafts.get(draftId);
    if (!d) throw new Error('this message was sent or deleted already');
    const label = where.page === 'chain' ? `chain ${where.chainId}, its ${where.tab} tab${where.cardId ? `, with agent ${where.cardId} open` : ''}` : `the ${where.page} page`;
    const e = ev(ASSISTANT, ASSISTANT_CARD, 'prompt', Date.now(), { by: OWNER, status: asCard.status === 'running' ? 'queued' : 'delivered', title: 'Your message', text: `${d.text}`, data: { placed: { where, label } } });
    asEvents.push(e);
    drafts.end([draftId]);
    fake.push('eventAdded', e);
    return e;
  },
  cardRestart: async (cardId) => {
    calls.push({ cardRestart: cardId });
    if (cardId === ASSISTANT_CARD) {
      asCard = { ...asCard, status: 'stopped', sessionId: '', activity: '', rev: asCard.rev + 1 };
      asAgent = { ...assistantStanding('ready'), rev: asAgent.rev + 100 };
      const e = ev(ASSISTANT, ASSISTANT_CARD, 'system', Date.now(), { ...startedOver(), status: 'new_session', by: OWNER, title: 'Started over', text: 'The next message opens a new session, which reads a recap of this conversation.' });
      asEvents.push(e);
      fake.push('cardUpdated', asCard);
      fake.push('agentStandingUpdated', asAgent);
      fake.push('eventAdded', e);
    }
    return {};
  },
  cardCancel: async (cardId) => (calls.push({ cardCancel: cardId }), {}),
  cardProcesses: async () => processesOf(),
  cardTelemetry: async () => telemetryOf(),
  cardContext: async (cardId) => {
    const k = chains.flatMap(cardsOf).find((x) => x.id === cardId)!;
    return contextOf(k, { given: givenOf(k.agent) });
  },
  repoViews: async () => repoViews(),
  repoGet: async (repo) => repoViews().find((v) => v.repo === repo) ?? { repo, queues: [], releases: [] },
  landingDiff: async (landingId) => ({ landingId, from: 'abc', files: [{ path: 'internal/store/store.go', added: 3, removed: 1, diff: '@@ -10,3 +10,6 @@\n func (s *Store) Chain(id string) (Chain, error) {\n-\treturn s.get(id)\n+\tc, err := s.get(id)\n+\tif err != nil {\n+\t\treturn c, fmt.Errorf("chain %s: %w", id, err)\n+\t}\n }\n' }] }),
  cardPrompt: async (draftId) => {
    if (params.get('fail')) throw new Error('the agent has no session to resume');
    const d = drafts.get(draftId);
    if (!d) throw new Error('this message was sent or deleted already');
    const e = ev(d.home, d.cardId!, 'prompt', Date.now(), { by: OWNER, status: 'queued', title: 'Your message', text: `${d.text}` });
    drafts.end([draftId]);
    fake.push('eventAdded', e);
    return e;
  },
  inboxAnswer: async (itemId, option, text) => {
    const it = inbox.find((i) => i.id === itemId)!;
    calls.push({ inboxAnswer: [itemId, option, text ? '<typed>' : ''] }); // what a typed value was is not kept, even here
    // A credential's value is sealed by the server and the variable then shows as set (a plain one with its value); removing a
    // connection drops it from the source. ?fail=1 refuses the value, quoting it.
    if (it.kind === 'credential') {
      if (params.get('fail')) throw new Error(`the value ${text} is not accepted for ${it.variable}`);
      const source = sources.find((s) => s.id === it.source)!;
      const edit = (connection: ToolSource['connections'][number]) => {
        if (option === 'remove-connection') return null;
        const variables = connection.variables.map((v) => (v.name === it.variable ? { ...v, state: 'set' as const, ...(v.secret ? {} : { value: text }) } : v));
        const missing = variables.filter((v) => !v.optional && v.state === 'unset').map((v) => v.name);
        return { ...connection, variables, missing, configured: missing.length === 0 && !connection.problem };
      };
      const connections = source.connections.flatMap((c) => (c.name === it.connection ? [edit(c)] : [c])).filter((c) => c !== null);
      const updated = { ...source, connections };
      sources = sources.map((s) => (s.id === updated.id ? updated : s));
      fake.push('sourceUpdated', updated);
    }
    // A change the owner lets through is written as shown: the record moves, and its history has the write, let through by him.
    if (it.kind === 'change' && option === 'apply') {
      const record = it.id === 'i10' ? agentsSeed().find((a) => a.id === 'researcher')! : playbooks.find((p) => p.id === 'bugfix')!;
      const written: Change = changeRecord({ id: `ch_${it.id}`, kind: it.id === 'i10' ? 'agent' : 'playbook', name: it.id === 'i10' ? 'researcher' : 'bugfix', action: 'changed', by: agent(it.cardId), approvedBy: OWNER, at: iso(Date.now()),
        fields: it.id === 'i10' ? [{ name: 'instructions', before: '"Search the index with the search tool and quote what it returns."', after: '"Search the index with the search tool, quote what it returns, and when it returns nothing, widen the query twice before you say so."' }] : [{ name: 'capabilities', before: '["history","issues"]', after: '["history","issues","library","playbooks"]' }] });
      changeHistory[`${written.kind}/${written.name}`] = [written, ...(changeHistory[`${written.kind}/${written.name}`] ?? [])];
      fake.push(it.id === 'i10' ? 'agentUpdated' : 'playbookUpdated', { ...record, rev: record.rev + 1 } as never);
    }
    fake.push('inboxUpdated', { ...it, status: 'answered', rev: it.rev + 1 });
    return {};
  },
  feedbackRead: async (id) => {
    const f = { ...feedback.find((x) => x.id === id)!, readAt: iso(Date.now()) };
    fake.push('feedbackUpdated', { ...f, rev: f.rev + 1 });
    return f;
  },
  chainArchive: async (id) => {
    const c = chains.find((x) => x.id === id)!;
    fake.push('chainUpdated', readOf({ ...c, archivedAt: iso(Date.now()), rev: c.rev + 1 }));
    return {};
  },
  chainDelete: async (id) => (fake.push('chainDeleted', { id }), {}),
  chainCreate: async (input) => {
    if (params.get('fail')) throw new Error('the server is busy');
    calls.push({ chainCreate: input });
    return newChain(input.intent, input.project);
  },
  healthCheck: async () => ({ ok: true, error: '', accounts: [accountHealth({ id: 'default', kind: 'machine', ok: true })], checkedAt: iso(Date.now()), unreadable: 0, rev: 2 }),
});

// A chain asked for in words, as the server makes it: a work chain queued under the first line of the words, then started with its Chair, which titles it in its first turn (set_title). ?title=<ms> sets how long
// the Chair takes to title it (default 1,500). Asked from a board narrowed to a project, it is on that project.
function newChain(intent: string, project?: string): Chain {
  const t = Date.now();
  const first = intent.split('\n')[0].trim();
  const c = chain(`new${t}`, first.slice(0, 80), 'queued', 0, { goal: intent });
  chains.push(c);
  // Each change is a new record, as the server sends: the replica holds what it was given and keeps the higher rev.
  const update = (change: Partial<Chain>): Chain => {
    const i = chains.findIndex((x) => x.id === c.id);
    chains[i] = { ...chains[i], ...change, rev: chains[i].rev + 1 };
    return chains[i];
  };
  madeCards.set(c.id, []);
  madeDecisions.set(c.id, []);
  histories.set(c.id, []);
  fake.push('chainUpdated', readOf(c));
  // Where it stands, as the server pushes it: first in the queue's column, then running.
  const stand = (to: 'queued' | 'running') => {
    const cols = board.columns.map((col) => ({ ...col, chains: col.chains.filter((id) => id !== c.id) }));
    cols.find((col) => col.key === to)!.chains.unshift(c.id);
    board = { ...board, columns: cols, rev: board.rev + 100 };
    fake.push('boardUpdated', board);
    fake.push('chainStandingUpdated', to === 'queued' ? stands(c.id, 'Waiting for its turn', 'queued', { place: 1, project, rev: Date.now() }) : stands(c.id, 'Running', 'running', { working: true, project, rev: Date.now() }));
  };
  stand('queued');
  setTimeout(() => {
    const chair = card(c.id, c.chairId, 'chair', 'Chair', { status: 'running', createdAt: iso(Date.now()), turns: 1, plan: [], activity: 'Titling the chain…' });
    const running = update({ status: 'running' });
    madeCards.get(c.id)!.push(chair);
    histories.get(c.id)!.push(ev(c.id, chair.id, 'prompt', Date.now(), { by: REMIT, status: 'delivered', text: `GOAL:\n${intent}` }));
    fake.push('cardUpdated', chair);
    fake.push('chainUpdated', readOf(running));
    for (const a of agentsOf(running)) fake.push('agentStandingUpdated', a);
    stand('running');
    setTimeout(() => {
      const words = first.split(/\s+/).slice(0, 6).join(' ');
      const title = words.charAt(0).toUpperCase() + words.slice(1);
      const titled = decisionRead(decisionRecord({ id: `${c.id}-title`, chainId: c.id, kind: 'title', by: agent(chair.id), cardId: chair.id, text: `Title the chain "${title}".`, why: 'It names what the chain does.', createdAt: iso(Date.now()) }), false);
      madeDecisions.get(c.id)!.push(titled);
      fake.push('decisionAdded', titled);
      fake.push('chainUpdated', readOf(update({ title })));
    }, Number(params.get('title') ?? 1500));
  }, 500);
  return { ...c };
}

signedIn();
const router = appRouter(createMemoryHistory());
await router.push(params.get('at') ?? '/board');
createApp(Shell).use(router).provide(apiKey, api).mount('#app');
// For looking at a lost connection: window.lose() plays the session going under (the core dials again); for looking at a
// change that arrives while a page is open: window.push(name, arg) is the server pushing it; window.asked lists the calls
// the page made; window.where() is the address the app is at (its history is in memory, not the page's).
Object.assign(window, { router, lose: fake.lose, push: fake.push, asked: fake.asked, where: () => router.currentRoute.value.fullPath });

// The busy chain's engineer (or ?to=) keeps writing, as a streaming agent does.
if (!params.get('still')) {
  const writing = histories.get('busy')!.at(-1)!;
  let written = writing.event.text.length;
  const chunk = ' · one more zone';
  setInterval(() => {
    fake.push('eventDelta', { id: writing.event.id, home: 'busy', cardId: writer, kind: 'message', text: chunk, offset: written });
    // The server keeps the message as it grows, so a page that reads it late reads it up to the next chunk.
    writing.event.text += chunk;
    written += chunk.length;
  }, params.get('chunks') ? 1000 / Number(params.get('chunks')) : 700);
}
// The Assistant writes its answer while it works.
if (!params.get('still') && asState === 'working') {
  const writing = asEvents.at(-1)!;
  let written = writing.event.text.length;
  const words = [' (ch_busy)', ' to have QC', ' re-check the', ' last 3 zones.', ' I will tell you', ' what it says.'];
  let n = 0;
  setInterval(() => {
    const chunk = words[n++ % words.length];
    fake.push('eventDelta', { id: writing.event.id, home: ASSISTANT, cardId: ASSISTANT_CARD, kind: 'message', text: chunk, offset: written });
    writing.event.text += chunk;
    written += chunk.length;
  }, 900);
}
// A new question arrives in the busy chain ?wait=<ms> after load, unseen: the board's list grows by one (and a browser
// notification shows, once the owner has turned them on and is not looking at the page).
if (params.get('wait')) {
  setTimeout(() => {
    const item = inboxRecord({ id: 'i_new', home: 'busy', cardId: 'eng', kind: 'question', title: 'Backfill from 2025-01 or only this year?', detail: 'The zones table starts in 2023.', status: 'open', createdAt: new Date().toISOString() });
    fake.push('inboxUpdated', item);
    // Newest of what holds an agent up: last in the list (the server's order).
    board = { ...board, waiting: [...board.waiting, itemWait(item, 'Engineer', true, { unseen: true })], rev: board.rev + 100 };
    fake.push('boardUpdated', board);
  }, Number(params.get('wait')));
}
if (params.get('arrive')) {
  const tools = [
    { title: 'Read internal/orch/zones.go', ...tool('read', 'read', 'internal/orch/zones.go') },
    { title: 'Grep zone_group', ...tool('search', 'search', 'zone_group') },
    { title: 'Edit internal/orch/zones.go', ...tool('edit', 'edit', 'internal/orch/zones.go') },
  ];
  let n = 0;
  setInterval(() => fake.push('eventAdded', ev('busy', writer, 'tool', Date.now(), { ...tools[n++ % tools.length], status: 'completed', data: { durationMs: 200 } })), Number(params.get('arrive')));
}
