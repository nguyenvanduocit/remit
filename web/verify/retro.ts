// A chain's page on its Live feed as the owner returns to it, on made-up data: over the feed, what happened while he was away (a
// long list: 8 lines and 5 more); in the feed, in the order things happened, the runs of its agents (many short turns, a long
// idle gap, a group of tool calls with two failed), its outcomes with who and why (paused, resumed, an agent stopped, merged,
// shipped, live, finished) between them, and last the coach's retrospective with several issues, memories that helped and
// misled, a better brief and the lesson it proposed, waiting on him with Apply and Not now. No server, no sign-in.
// Open /verify/retro.html; ?status=running|paused|done|archived picks where the chain stands (default done: finished, so Archive
// is offered; running offers Pause, which pauses it once confirmed; paused: paused by the owner with a why, its header saying so,
// Resume offered; archived offers Unarchive); ?away=0 opens it with nothing new; ?push=1 has the
// retrospective arrive two seconds after the page opens (retrospective_added). The calls the buttons make are in window.calls.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import { createMemoryHistory, createRouter } from 'vue-router';
import ChainPage from '../src/components/ChainPage.vue';
import { TooltipProvider } from '../src/components/ui/tooltip';
import { Toaster } from '../src/components/ui/sonner';
import { apiKey } from '../src/core/api';
import { opened, openChain } from '../src/openChain';
import { api } from '../src/replica';
import { routes } from '../src/router';
import { fakeServer, snapshotOf } from './fake';
import { agentStands, stands } from './standingSeed';
import { tool, turnEnd } from './activitySeed';
import type { Actor, Away, Card, ChainStanding, DecisionKind, DecisionRead, EventKind, EventRead, RetrospectiveRead } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { actorOf, cardRecord, chainRead, chainRecord, decisionRead, decisionRecord, inboxRecord, optionOf, retrospectiveRead, seedEvent, type EventSeed } from './records';

const params = new URLSearchParams(location.search);
const status = params.get('status') ?? 'done';
const MIN = 60_000;
const now = Date.now();
const t0 = now - 6 * 60 * MIN;
const iso = (ms: number) => new Date(ms).toISOString();
const owner: Actor = actorOf('owner', 'alexp', 'web');
const chairBy: Actor = actorOf('agent', 'chair');
const coach: Actor = actorOf('agent', 'coach-chair');

const chain = chainRecord({ id: 'ch', title: 'Backfill missing zone groups', goal: 'Backfill the zone groups the March import dropped, in the shop timezone.', status: status === 'running' ? 'running' : status === 'paused' ? 'paused' : 'done', chairId: 'chair', workdir: '/', origin: 'owner', createdAt: iso(t0), updatedAt: iso(now), ...(status === 'archived' && { archivedAt: iso(now - 10 * MIN) }) });
const card = (id: string, agent: string, name: string, extra: Partial<Card> = {}): Card =>
  cardRecord({ id, home: 'ch', agent, name, origin: agent === 'chair' ? 'chair' : 'hired', model: 'opus', mode: 'auto', cwd: '/', sessionId: 's-' + id, status: status === 'running' && id !== 'rv' ? 'running' : 'idle', plan: [], ctxUsed: 60_000, ctxSize: 200_000, turns: 12, queued: 0, createdAt: iso(t0), updatedAt: iso(now), rev: 1, ...extra });
const cards = [card('chair', 'chair', 'Chair'), card('eng', 'engineer', 'Engineer · zone groups backfill'), card('rv', 'reviewer', 'Reviewer')];

let id = 0;
const ev = <K extends EventKind>(cardId: string, kind: K, min: number, extra: EventSeed<K> = {}): EventRead =>
  seedEvent({ id: ++id, home: 'ch', cardId, createdAt: iso(t0 + min * MIN), updatedAt: iso(t0 + min * MIN + 3000) }, kind, extra);
const turn = (cardId: string, min: number, said: string, tools: EventRead[] = []) => [
  ev(cardId, 'prompt', min, { by: chairBy, status: 'delivered', title: 'Next step', text: '[message from the Chair]\nNext step.' }),
  ...tools,
  ev(cardId, 'message', min + 0.6, { text: said }),
  ev(cardId, 'stop', min + 0.7, { ...turnEnd(40_000), status: 'end_turn' }),
];
const events: EventRead[] = [
  ...turn('chair', 2, 'Hired an engineer to backfill the zone groups.'),
  ...turn('eng', 5, 'The migration is ready; two tests still fail, retrying with a fixed clock.', [
    ev('eng', 'tool', 5.1, { title: 'cargo test', ...tool('execute', 'cargo', 'test retention_keeps_90_days', 'failed') }),
    ev('eng', 'tool', 5.2, { title: 'cargo test', ...tool('execute', 'cargo', 'test zone_groups_in_shop_tz', 'failed') }),
    ev('eng', 'tool', 5.3, { title: 'Read', ...tool('read', 'Read', 'crates/store/src/zones.rs') }),
    ev('eng', 'tool', 5.4, { title: 'Edit', ...tool('edit', 'Edit', 'crates/store/src/zones.rs') }),
  ]),
  // Many short turns of the reviewer.
  ...Array.from({ length: 6 }, (_, i) => turn('rv', 20 + i * 2, `Round ${i + 1}: re-read zones.rs, ${i % 2 ? 'no change' : 'one comment'}.`, [ev('rv', 'tool', 20.1 + i * 2, { title: 'Read', ...tool('read', 'Read', 'crates/store/src/zones.rs') })])).flat(),
  // A long idle gap, then the fix and the merge.
  ...turn('eng', 200, 'Tests pass in UTC+7 now: the test reads shops.tz.', [ev('eng', 'tool', 200.2, { title: 'cargo test', ...tool('execute', 'cargo', 'test -p store') })]),
  ...turn('chair', 240, 'Merged !12 in stages; the change is live.'),
];

// `outcome` as the server marks it: an outcome of the chain's work shows in its stream.
const d = (kind: DecisionKind, outcome: boolean, min: number, by: Actor, text: string, why = ''): DecisionRead => decisionRead(decisionRecord({ id: `d_${kind}_${min}`, chainId: 'ch', kind, by, text, why, cardId: 'chair', createdAt: iso(t0 + min * MIN) }), outcome);
const decisions: DecisionRead[] = [
  d('hire', false, 2.5, chairBy, 'Hire engineer: zone groups backfill.'),
  d('pause', true, 30, owner, 'Pause chain.', 'the reviewer keeps re-reading unchanged files'),
  d('resume', true, 190, owner, 'Resume chain.'),
  d('stop', true, 195, chairBy, 'Stop Reviewer.', 'six rounds with no new finding'),
  d('accept', false, 236, chairBy, 'Accept the engineer report.'),
  d('landing', true, 241, actorOf('remit'), 'Landed fix(backfill): zone groups as a stage at 5%.', 'checkout errors 0.2%, under the 1% guard'),
  d('landing', true, 250, actorOf('remit'), 'Landed fix(backfill): zone groups at 100%.', 'guarded by checkout errors until 22:00'),
  d('live', true, 252, actorOf('remit'), 'The change is live.'),
  ...(status === 'running' || status === 'paused' ? [] : [d('finish', true, 255, chairBy, 'Finish chain.', 'the zone groups are back and the tests read the shop timezone')]),
  ...(status === 'paused' ? [d('pause', true, 258, owner, 'Pause chain.', 'wait for the March export before the counts are compared')] : []),
  ...(status === 'archived' ? [d('archive', true, 350, owner, 'Archive chain.', 'duplicate of Backfill zone groups (March)')] : []),
];

const lesson = inboxRecord({
  id: 'i_lesson', home: 'ch', cardId: '', kind: 'change', changeKind: 'promote', title: 'A lesson for the Engineer seat',
  detail: 'Before writing a time-based test, read the shop\'s timezone from `shops.tz`.',
  options: [optionOf('apply', 'Accept as lesson'), optionOf('later', 'Not now')], status: 'open', createdAt: iso(t0 + 262 * MIN), rev: 1,
});
const retro: RetrospectiveRead = retrospectiveRead({
  id: 'r1', chainId: 'ch', by: coach, createdAt: iso(t0 + 260 * MIN), rev: 1, lesson: lesson.id,
  issues: [
    { impact: 'high', text: '2 rounds lost: the tests ran in UTC and the shop runs in UTC+7.' },
    { impact: 'medium', text: 'The reviewer re-read unchanged files for six rounds before the Chair stopped it.' },
    { impact: 'medium', text: 'The chain sat paused for 2 h 40 min with nothing waiting on the owner.' },
    { impact: 'low', text: 'The first brief named Mongo, where the zone groups no longer live.' },
  ],
  memories: [
    { memoryId: 'm1', text: 'zone groups live in StarRocks, not Mongo', helped: true, why: 'it sent the engineer to the right table at once' },
    { memoryId: 'm2', text: 'backfills run on weekends', helped: false, why: 'no longer true since the September move; it delayed the merge a day' },
    { memoryId: 'm3', text: 'the store tests take a fixed clock from TEST_NOW', helped: true, why: 'the fix used it' },
  ],
  brief: 'Backfill the zone groups the March import dropped. They live in StarRocks (`zone_groups`). Every time-based test reads the shop\'s timezone from `shops.tz`. Done when the backfill is merged and the counts match the March export.',
}, 'Coach');
const away: Away = {
  since: iso(now - 42 * MIN),
  more: 5,
  lines: [
    { at: iso(t0 + 236 * MIN), who: chairBy, text: 'Accepted the engineer report.' },
    { at: iso(t0 + 238 * MIN), who: chairBy, text: 'Opened !12 on g/zones.' },
    { at: iso(t0 + 241 * MIN), who: chairBy, text: 'Merged !12: stage 5% · checkout errors 0.2%, under the 1% guard.' },
    { at: iso(t0 + 245 * MIN), who: actorOf('assistant', 'assistant'), text: 'Answered the Chair for you: keep the 1% guard until 22:00.' },
    { at: iso(t0 + 250 * MIN), who: chairBy, text: 'Merged !12: 100%, guarded by checkout errors until 22:00.' },
    { at: iso(t0 + 252 * MIN), who: actorOf('remit'), text: 'The change is live.' },
    { at: iso(t0 + 255 * MIN), who: chairBy, text: 'Finished the chain: the zone groups are back and the tests read the shop timezone, so the next backfill starts from a brief that says so.' },
    { at: iso(t0 + 258 * MIN), who: chairBy, text: 'Answered every word the owner left on the change.' },
  ],
};

const ASKS: Record<string, ChainStanding['asks']> = { running: ['pause', 'finish'], paused: ['resume', 'finish'], done: ['archive', 'delete'], archived: ['unarchive', 'delete'] };
const standing = (asks = ASKS[status] ?? ASKS.done) =>
  status === 'running' ? stands('ch', 'Running', 'running', { working: true, asks }) : status === 'paused' ? stands('ch', 'Paused', 'paused', { asks }) : stands('ch', 'Done', 'done', { asks });
const agents = cards.map((c) => agentStands('ch', c.id, c.status === 'running' ? 'Running' : 'Done', c.status === 'running' ? 'running' : 'done', c.status === 'running' ? 'Working…' : 'Turn done', c.status === 'running' ? 'busy' : 'done'));
const pushRetro = params.get('push') === '1';

const calls: string[] = [];
(window as unknown as { calls: string[] }).calls = calls;
// An action on the whole chain, as the server answers it: the decision lands in the stream with who and why, the standing moves on.
const acted = (kind: DecisionKind, text: string, asks: ChainStanding['asks']) => (chainId: string, why: string) => {
  calls.push(`${kind} ${chainId} ${JSON.stringify(why)}`);
  fake.push('decisionAdded', decisionRead(decisionRecord({ id: `d_${kind}_${Date.now()}`, chainId, kind, by: owner, text, why, createdAt: iso(Date.now()) }), true));
  fake.push('chainStandingUpdated', standing(asks));
  return {};
};
const fake = fakeServer({
  appSnapshot: () => snapshotOf({ chains: [chainRead(chain)], inbox: [lesson], standings: [standing()] }),
  chainGet: () => ({ chain: chainRead(chain), cards, edges: [], decisions, datadog: [], landings: [], drafts: [], workflows: [], helps: [], agents, verification: { chainId: 'ch', gates: [], judges: [], rev: 1 }, standing: standing(), retrospectives: pushRetro ? [] : [retro], ...(params.get('away') !== '0' && { away }) }),
  chainEvents: () => ({ events, truncated: false }),
  cardEvents: (cardId) => ({ events: events.filter((e) => e.event.cardId === cardId), truncated: false }),
  chainPause: (chainId, why) => (acted('pause', 'Pause chain.', ['resume'])(chainId, why), fake.push('chainUpdated', chainRead({ ...chain, status: 'paused', rev: 2 })), {}),
  chainArchive: acted('archive', 'Archive chain.', ['unarchive', 'delete']),
  chainUnarchive: acted('unarchive', 'Unarchive chain.', ['archive', 'delete']),
  chainResume: (chainId) => acted('resume', 'Resume chain.', ['pause', 'finish'])(chainId, ''),
  chainCreate: (input) => (calls.push(`chainCreate ${JSON.stringify(input.intent)}`), { ...chain, id: 'ch2', title: 'Backfill missing zone groups (again)', status: 'queued' }),
  inboxAnswer: (itemId, optionId, text) => (calls.push(`inboxAnswer ${itemId} ${optionId} ${JSON.stringify(text)}`), fake.push('inboxUpdated', { ...lesson, status: 'answered', rev: 2 }), {}),
});
openChain('ch');
if (pushRetro) setTimeout(() => fake.push('retrospectiveAdded', retro), 2000);

const router = createRouter({ history: createMemoryHistory(), routes });
await router.push({ name: 'chain', params: { id: 'ch', tab: 'live' } });
createApp({ render: () => h(TooltipProvider, { delayDuration: 300 }, () => [h('div', { class: 'flex h-screen flex-col' }, [opened.view && h(ChainPage, { view: opened.view })]), h(Toaster)]) })
  .use(router)
  .provide(apiKey, api)
  .mount('#app');
