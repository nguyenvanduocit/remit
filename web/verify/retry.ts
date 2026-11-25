// The Chair's stream after a turn the usage limit ended, and the owner's retry. No server, no sign-in. Open /verify/retry.html.
// The stream holds what the server stored: the message, the work before the failure, the agent's own words about the
// limit and the error. "Retry" plays the server's side of the owner's retry in the order it does it (the message is
// queued again, the failure's records are deleted, the turn starts), through the page's real client core on a fake server.
// ?w=<px> sets the width of the column.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { computed, createApp, h, ref } from 'vue';
import { apiKey } from '../src/core/api';
import StreamBlocks from '../src/components/StreamBlocks.vue';
import { TooltipProvider } from '../src/components/ui/tooltip';
import { opened, openChain, useCardEvents } from '../src/openChain';
import { api } from '../src/replica';
import { blocks } from '../src/stream';
import { fakeServer, snapshotOf } from './fake';
import type { InboxItem, PromptStatus } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { stands } from './standingSeed';
import { tool } from './activitySeed';
import { actorOf, cardRecord, chainRead, chainRecord, inboxRecord, optionOf, retryOf, seedEvent } from './records';

const width = new URLSearchParams(location.search).get('w') ?? '520';
const MIN = 60_000;
const t0 = Date.now() - 12 * MIN;
const iso = (ms: number) => new Date(ms).toISOString();

const chair = cardRecord({ id: 'chair', home: 'ch', agent: 'chair', name: 'Chair', origin: 'chair', model: 'opus', mode: 'auto', cwd: '/', status: 'idle', plan: [], ctxUsed: 0, ctxSize: 0, turns: 1, queued: 0, createdAt: iso(t0), updatedAt: iso(t0), rev: 1 });
const chain = chainRecord({ id: 'ch', title: 'x', goal: 'x', status: 'running', chairId: 'chair', workdir: '/', createdAt: iso(t0), updatedAt: iso(t0), rev: 1 });
const limit = "You've hit your session limit · resets 9:40am (UTC)";
const at = (id: number, secs: number) => ({ id, home: 'ch', cardId: 'chair', createdAt: iso(t0 + secs * 1000), updatedAt: iso(t0 + secs * 1000) });
// The failed turn's message, at each revision the server sends of it.
const prompt = (status: PromptStatus, rev: number) =>
  seedEvent(at(1, 0), 'prompt', { title: 'Resume after a restart', text: 'Remit restarted at about 08:51Z and your session stopped. Please resume: check the engineer’s state and carry on to the MR.', by: actorOf('owner', 'alexp'), status, rev });
const failed = [
  prompt('delivered', 2),
  seedEvent(at(2, 5), 'message', { text: 'Checking the engineer’s state before I re-brief it.' }),
  seedEvent(at(3, 8), 'tool', { ...tool('remit', 'list_agents', 'Remit'), status: 'completed', title: 'mcp__remit__list_agents' }),
  seedEvent(at(4, 20), 'message', { text: limit }),
  seedEvent(at(5, 20), 'system', { status: 'error', text: `{"code":-32603,"message":"Internal error: ${limit}","data":{"errorKind":"rate_limit"}}` }),
];

// The error item the failure raised, as the server writes it: its title says the failure, and it names the events that
// record it, which the stream then shows once, as the item.
const errorText = failed[4].event.text;
const item = ref<InboxItem[]>([inboxRecord({ id: 'i_1', home: 'ch', cardId: 'chair', kind: 'error', title: 'Chair failed: ' + errorText.slice(0, 99) + '…', detail: errorText,
  options: [optionOf('retry', 'Retry'), optionOf('dismiss', 'Dismiss')], status: 'open', retry: retryOf({ cardId: 'chair', promptId: 1, failureIds: [4, 5] }), createdAt: iso(t0 + 20_000) })]);

const fake = fakeServer({
  appSnapshot: () => snapshotOf({ chains: [chainRead(chain)] }),
  chainGet: () => ({ chain: chainRead(chain), cards: [chair], edges: [], retrospectives: [], decisions: [], helps: [], datadog: [], landings: [], drafts: [], workflows: [], agents: [], verification: { chainId: 'ch', gates: [], judges: [], rev: 1 }, standing: stands('ch', 'Running', 'running') }),
  chainEvents: () => ({ events: failed, truncated: false }),
});
openChain('ch');

// The server's side of the retry, in its order: the message is queued again, the failure goes, the turn starts.
function retry(): void {
  item.value = [];
  fake.push('eventUpdated', prompt('queued', 3));
  fake.push('eventDeleted', { id: 4, home: 'ch', cardId: 'chair' });
  fake.push('eventDeleted', { id: 5, home: 'ch', cardId: 'chair' });
  fake.push('eventUpdated', prompt('delivered', 4));
  fake.push('eventAdded', seedEvent(at(6, 700), 'message', { text: 'Resuming from where I stopped: the engineer is still running, so I am carrying on to the MR.' }));
}

const chairEvents = useCardEvents('chair');
const stream = computed(() => blocks(chairEvents.value, item.value));
createApp({
  render: () =>
    h(TooltipProvider, { delayDuration: 300 }, () =>
      h('div', { class: 'p-4', style: { width: width + 'px' } }, [
        h('button', { 'data-testid': 'retry', class: 'mb-3 rounded border border-line px-3 py-1 text-xs text-fg', onClick: retry }, 'Retry'),
        opened.view ? h(StreamBlocks, { blocks: stream.value, live: false, cardId: 'chair' }) : null,
      ]),
    ),
})
  .provide(apiKey, api)
  .mount('#app');
