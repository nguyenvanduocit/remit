// An agent's activity (CardActivity) in four moments, side by side, on made-up data. No server, no sign-in. Open
// /verify/activity.html.
// Left: the tool is done and the agent has said nothing yet (the typing line). Second: it is writing its thinking (the
// Thinking block, open, and no typing line). Third: the thinking is over and the next tool runs (the typing line is back
// once that tool is done). Fourth: it asked for a credential, the owner answered in words instead and the agent works on:
// the ask stays where it was raised, between the two, and the typing line is there. Fifth: a QC approved an earlier version
// and the agent kept working: the verdict stays where it was given, with the steps after it below it.
// ?w=<px> sets the width of each column (420 is the drawer).
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import { apiKey } from '../src/core/api';
import { api } from '../src/replica';
import CardActivity from '../src/components/CardActivity.vue';
import { TooltipProvider } from '../src/components/ui/tooltip';
import { fakeServer, snapshotOf } from './fake';
import type { AgentStanding, Card, EventKind, EventRead, InboxItem, Judge } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { tool } from './activitySeed';
import { actorOf, cardRecord, inboxRecord, seedEvent, type EventSeed } from './records';

const width = new URLSearchParams(location.search).get('w') ?? '520';
const t0 = Date.now() - 5 * 60_000;
const iso = (ms: number) => new Date(ms).toISOString();

const card = (id: string): Card =>
  cardRecord({ id, home: 'ch', agent: 'engineer', name: 'Engineer', origin: 'hired', model: 'opus', mode: 'auto', cwd: '/', status: 'running', plan: [], ctxUsed: 0, ctxSize: 0, turns: 1, queued: 0, createdAt: iso(t0), updatedAt: iso(t0), rev: 1 });
const stands = (cardId: string, doing: string): AgentStanding => ({ cardId, home: 'ch', label: 'Running', tone: 'running', doing, doingTone: 'busy', work: '', warms: true, waiting: [], children: [], with: [], peers: [], branch: { agents: 0, turns: 0, working: true }, asks: ['message'], stopSays: '', remembers: false, rev: 1 });

let n = 0;
const ev = <K extends EventKind>(cardId: string, kind: K, extra: EventSeed<K> = {}): EventRead =>
  seedEvent({ id: ++n, home: 'ch', cardId, createdAt: iso(t0 + n * 8000), updatedAt: iso(t0 + n * 8000 + 3000) }, kind, extra);
const grep = (cardId: string) =>
  ev(cardId, 'tool', { ...tool('search', 'search', 'rg -n "debounced syncs" internal'), status: 'completed', title: 'rg -n "debounced syncs" internal', toolCallId: 'tc' + n, data: { input: { command: 'rg -n "debounced syncs" internal' }, output: '485:\th.logger.Warn("stop: flushing debounced syncs, orders already acked",\n435:\th.stopping.Store(true)' } });
const thinking = 'The grep shows the flush at line 485 runs after `stopping` is set at line 435, so a sync debounced during the stop is flushed once.\n\nNext: read `Stop` in full to see whether the flush can run twice.';

const left = [ev('a', 'prompt', { title: 'Fix the stop path', text: '[handoff from the Chair]\nFind out why a stop flushes the debounced syncs twice.', by: actorOf('agent', 'chair'), status: 'delivered' }), grep('a')];
const middle = [ev('b', 'prompt', { title: 'Fix the stop path', text: '[handoff from the Chair]\nFind out why a stop flushes the debounced syncs twice.', by: actorOf('agent', 'chair'), status: 'delivered' }), grep('b'), ev('b', 'thought', { text: thinking })];
const right = [
  ev('c', 'prompt', { title: 'Fix the stop path', text: '[handoff from the Chair]\nFind out why a stop flushes the debounced syncs twice.', by: actorOf('agent', 'chair'), status: 'delivered' }),
  grep('c'),
  ev('c', 'thought', { text: thinking }),
  ev('c', 'tool', { ...tool('read', 'read', 'handler.go'), status: 'completed', title: 'Read handler.go', toolCallId: 'tc' + n }),
];

// The ask is raised after the agent's first message and before the owner's reply, whose message the agent is working on.
const asked = [
  ev('d', 'prompt', { title: 'Connect MongoDB', text: '[handoff from the Chair]\nConnect the staging MongoDB.', by: actorOf('agent', 'chair'), status: 'delivered' }),
  ev('d', 'message', { text: 'The **mongo** connection staging needs a value I must not read. I asked for it.' }),
];
const credential = inboxRecord({
  id: 'i_cred', home: 'ch', cardId: 'd', kind: 'credential', title: 'Credential for mongo/staging: MONGODB_URI', source: 'mongo', connection: 'staging', variable: 'MONGODB_URI', secret: true, options: [], status: 'open',
  detail: 'The **MongoDB** connection staging needs the connection string of the deployment (`MONGODB_URI`). Type it into the field of this item: it is stored encrypted, and no agent sees it.',
  createdAt: iso(t0 + n * 8000 + 4000), rev: 1,
});
const workedOn = [
  ...asked,
  ev('d', 'prompt', { text: 'Can you not look for it yourself? It is right on this machine, copy it in.', by: actorOf('owner', 'owner@example.com', 'web'), status: 'delivered' }),
  ev('d', 'tool', { ...tool('read', 'read', '~/.claude.json'), status: 'completed', title: 'Read ~/.claude.json', toolCallId: 'tc' + n }),
];

// The QC's verdict lands after the first tool and before the thinking that follows it.
const judgedMid = [
  ev('e', 'prompt', { title: 'QC · sales-engine bulk app name', text: '[handoff from the Chair]\nJudge the diff.', by: actorOf('agent', 'chair'), status: 'delivered' }),
  grep('e'),
];
const verdictAt = iso(t0 + n * 8000 + 4000);
const afterVerdict = [ev('e', 'thought', { text: thinking }), ev('e', 'tool', { ...tool('read', 'read', 'handler.go'), status: 'completed', title: 'Read handler.go', toolCallId: 'tc' + n })];
const qc: Judge = {
  key: 'qc|svc|qc', cardId: 'e', role: 'qc', name: 'QC · sales-engine bulk app name', earlier: [],
  latest: { id: 'v1', cardId: 'e', check: { label: 'Approved an earlier version', tone: 'needs' }, headline: 'QC verdict on HEAD b1fc8df. The diff touches 3 files and adds 89 lines.', rest: 'Ran the suite.\n\nAll green.', more: 2, findings: [], moves: [], at: verdictAt },
};

fakeServer({ appSnapshot: () => snapshotOf({}) });
const column = (id: string, events: EventRead[], doing: string, label: string, waiting: InboxItem[] = [], judged: Judge[] = []) =>
  h('div', { 'data-col': id, style: { width: width + 'px', flex: 'none' } }, [
    h('div', { class: 'mb-3 text-[11px] text-faint' }, label),
    h(CardActivity, { card: card(id), chair: false, events, agent: stands(id, doing), waiting, judged }),
  ]);
createApp({
  render: () =>
    h(TooltipProvider, { delayDuration: 300 }, () =>
      h('div', { class: 'flex items-start gap-6 p-4' }, [
        column('a', left, 'Thinking about the next step…', 'tool done, nothing yet'),
        column('b', middle, 'Thinking…', 'writing its thinking'),
        column('c', right, 'Thinking about the next step…', 'moved on to the next tool'),
        column('d', workedOn, 'Thinking about the next step…', 'a credential asked, answered in words, the agent works on', [credential]),
        column('e', [...judgedMid, ...afterVerdict], 'Thinking about the next step…', 'a verdict given, the agent works on', [], [qc]),
      ]),
    ),
})
  .provide(apiKey, api)
  .mount('#app');
