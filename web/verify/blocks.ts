// An agent's stream with the blocks it showed (`show_block`, ADR-0091), on made-up data. No server, no sign-in. Open
// /verify/blocks.html.
// The engineer offers three ways to cut a migration as a choice, each with a line of detail, and goes on with its own
// pick (none of the owner's yet). A second block holds a `many` choice the owner picked, a two-series line chart and a bar
// chart with a unit. A pick calls `block_pick`; this page answers it with the block as the server would store it and pushes
// the event updated, as the server does, so the stream draws the replica's. A reply carries the teaching marks (`turn_teach`,
// kept in window.calls with the picks), answered the same way: the reply keeps the owner's mark, his new one replacing it. ?slow=<ms> answers a pick that late, to see it on its way.
// ?w=<px> sets the width of the column (420 is the drawer).
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import { apiKey } from '../src/core/api';
import type { Actor, Block, BlockPart, EventKind, EventRead, TurnMark } from '../src/core/generated/types.generated';
import StreamBlocks from '../src/components/StreamBlocks.vue';
import { TooltipProvider } from '../src/components/ui/tooltip';
import { opened, openChain } from '../src/openChain';
import { api } from '../src/replica';
import { blocks } from '../src/stream';
import { fakeServer, snapshotOf } from './fake';
import '../src/theme.css';
import { stands } from './standingSeed';
import { shownBlock, tool, turnEnd } from './activitySeed';
import { actorOf, cardRecord, chainRead, chainRecord, seedEvent, type EventSeed } from './records';

const params = new URLSearchParams(location.search);
const width = params.get('w') ?? '520';
const MIN = 60_000;
const t0 = Date.now() - 30 * MIN;
const iso = (ms: number) => new Date(ms).toISOString();

const card = cardRecord({ id: 'engineer', home: 'ch', agent: 'engineer', name: 'Engineer · orders migration', origin: 'hired', model: 'opus', mode: 'auto', cwd: '/', status: 'running', plan: [], ctxUsed: 0, ctxSize: 0, turns: 2, queued: 0, createdAt: iso(t0), updatedAt: iso(t0), rev: 1 });
const chair: Actor = actorOf('agent', 'chair');
const owner: Actor = actorOf('owner', 'alexp', 'web');

/** A choice of three, each with a line that compares it: the agent goes ahead with the first. */
export const migrationBlock: Block = {
  spec: {
    title: 'How to cut the orders migration',
    parts: [
      {
        type: 'choice', id: 'how', label: 'Approach', many: false, picked: ['one'], options: [
          { id: 'one', label: 'One step', detail: 'Fastest: one deploy, locks `orders` for about 40 s at 2 M rows.' },
          { id: 'two', label: 'Expand, then contract', detail: 'Two deploys a day apart; no lock, the old column stays readable meanwhile.' },
          { id: 'three', label: 'Copy and swap', detail: 'A shadow table filled in the background; the longest to run, and the swap is the only risk.' },
        ],
      },
    ],
  },
};

/** A `many` choice the owner picked, a two-series line chart, a bar chart with a unit and a two-series bar chart of changes, below zero and above. */
export const reviewBlock: Block = {
  spec: {
    title: 'Merge requests, last two weeks',
    parts: [
      {
        type: 'choice', id: 'repos', label: 'Repositories to watch', many: true, picked: ['api', 'web'], options: [
          { id: 'api', label: 'compass/api' },
          { id: 'web', label: 'compass/web', detail: 'Most of the open reviews.' },
          { id: 'jobs', label: 'compass/jobs', detail: 'Quiet: two merges in two weeks.' },
        ],
      },
      {
        type: 'chart', id: 'open', label: 'Open merge requests a day', kind: 'line',
        x: ['Nov 1', 'Nov 2', 'Nov 3', 'Nov 4', 'Nov 5', 'Nov 6', 'Nov 7', 'Nov 8', 'Nov 9', 'Nov 10', 'Nov 11', 'Nov 12', 'Nov 13', 'Nov 14'],
        series: [
          { name: 'opened', values: [4, 6, 3, 7, 5, 2, 1, 6, 8, 5, 4, 7, 6, 3] },
          { name: 'merged', values: [3, 4, 5, 4, 6, 1, 0, 5, 6, 7, 4, 5, 6, 4] },
        ],
      },
      {
        type: 'chart', id: 'wait', label: 'Time to first review', kind: 'bar', unit: 'h',
        x: ['compass/api', 'compass/web', 'compass/jobs'],
        series: [{ name: 'median', values: [3.5, 11.25, 1] }],
      },
      {
        type: 'chart', id: 'delta', label: 'Open reviews, change on last week', kind: 'bar',
        x: ['compass/api', 'compass/web', 'compass/jobs', 'compass/infra'],
        series: [{ name: 'this week', values: [-3, 5, -1.5, 0] }, { name: 'last week', values: [2, -4, 1, -0.5] }],
      },
    ],
  },
  pickedBy: owner,
  pickedAt: iso(t0 + 25 * MIN),
};

let n = 0;
const events: EventRead[] = [];
const ev = <K extends EventKind>(kind: K, mins: number, extra: EventSeed<K> = {}) =>
  events.push(seedEvent({ id: ++n, home: 'ch', cardId: 'engineer', createdAt: iso(t0 + mins * MIN), updatedAt: iso(t0 + mins * MIN) }, kind, extra));
ev('prompt', 0, { title: 'Add the status column to orders', text: '[handoff from the Chair]\nAdd `status` to `orders` without taking checkout down.', by: chair, status: 'delivered' });
ev('tool', 1, { ...tool('read', 'read', 'orders.sql'), status: 'completed', title: 'Read orders.sql' });
ev('message', 2, { text: 'Three ways to cut it. I go with one step unless you pick another.' });
ev('system', 3, { ...shownBlock(migrationBlock), status: 'block', title: migrationBlock.spec.title, by: actorOf('agent', 'engineer') });
ev('message', 4, { text: 'Writing the one-step migration now.' });
ev('system', 20, { ...shownBlock(reviewBlock), status: 'block', title: reviewBlock.spec.title, by: actorOf('agent', 'engineer'), updatedAt: iso(t0 + 25 * MIN), rev: 2 });
ev('stop', 21, { ...turnEnd(60_000), status: 'end_turn', data: { durationMs: 60_000 } });

/** The owner's pick, as `block_pick` stores it: the choice holds it, and the block says it is his. */
const calls: string[] = [];
(window as unknown as { calls: string[] }).calls = calls;
const slow = Number(params.get('slow') ?? 0);
const blockPick = async (eventId: number, part: string, picked: string[]): Promise<EventRead> => {
  calls.push(`blockPick ${eventId} ${part} ${picked.join(',')}`);
  if (slow) await new Promise((r) => setTimeout(r, slow));
  const e = events.find((x) => x.event.id === eventId);
  if (e?.activity?.type !== 'block') throw new Error(`event ${eventId} is no block`);
  const parts = e.activity.block.spec.parts.map((p): BlockPart => (p.type === 'choice' && p.id === part ? { ...p, picked } : p));
  const block: Block = { spec: { ...e.activity.block.spec, parts }, pickedBy: owner, pickedAt: new Date().toISOString() };
  const stored: EventRead = { event: { ...e.event, updatedAt: block.pickedAt!, rev: e.event.rev + 1 }, activity: { type: 'block', block } };
  events[events.indexOf(e)] = stored;
  fake.push('eventUpdated', stored);
  return stored;
};

/** The owner's mark on a reply, as `turn_teach` stores it: the reply keeps it, replacing his earlier one (ADR-0093). */
const turnTeach = (eventId: number, mark: TurnMark, words: string) => {
  calls.push(`turnTeach ${eventId} ${mark} ${JSON.stringify(words)}`);
  const e = events.find((x) => x.event.id === eventId);
  if (e?.event.kind !== 'message') throw new Error(`event ${eventId} is no reply`);
  const others = e.activity?.type === 'marked' ? e.activity.marks.filter((m) => m.mentor !== 'm_owner') : [];
  const at = new Date().toISOString();
  const stored: EventRead = { event: { ...e.event, updatedAt: at, rev: e.event.rev + 1 }, activity: { type: 'marked', marks: [...others, { mentor: 'm_owner', by: owner, mark, words, at }] } };
  events[events.indexOf(e)] = stored;
  fake.push('eventUpdated', stored);
  return {};
};

const chain = chainRecord({ id: 'ch', title: 'Orders status', goal: 'x', status: 'running', chairId: 'chair', workdir: '/', createdAt: iso(t0), updatedAt: iso(t0), rev: 1 });
const fake = fakeServer({
  appSnapshot: () => snapshotOf({ chains: [chainRead(chain)] }),
  chainGet: () => ({ chain: chainRead(chain), cards: [card], edges: [], retrospectives: [], decisions: [], helps: [], verification: { chainId: 'ch', gates: [], judges: [], rev: 1 }, datadog: [], landings: [], drafts: [], workflows: [], agents: [], standing: stands('ch', 'Running', 'running') }),
  chainEvents: () => ({ events, truncated: false }),
  blockPick,
  turnTeach,
});
openChain('ch');

createApp({
  render: () => h(TooltipProvider, { delayDuration: 300 }, () => h('div', { class: 'flex items-start gap-6 p-4' }, opened.view ? [h('div', { 'data-card': 'engineer', style: { width: width + 'px', flex: 'none' } }, [h(StreamBlocks, { blocks: blocks(opened.events), live: false })])] : [])),
})
  .provide(apiKey, api)
  .mount('#app');
