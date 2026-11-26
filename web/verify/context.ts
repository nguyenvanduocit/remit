// The card drawer of a busy agent (see contextSeed.ts), on made-up data: its header with the context meter, and the
// Session panel the meter opens, whose context section says what fills it. No server, no sign-in. Open
// /verify/context.html and press the meter, or ?panel=session to land on the panel; ?counted=none answers as for a
// session Claude Code has not counted yet, ?counted=stored as for one whose process did not answer now, ?loaded=none as
// for one whose first turn has not reported what it loaded, ?given=none as for one whose start Remit has not recorded, ?telemetry=none as for one that exported nothing yet,
// ?slow=1 answers after two seconds.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import { createMemoryHistory, createRouter } from 'vue-router';
import CardDrawer from '../src/components/CardDrawer.vue';
import { TooltipProvider } from '../src/components/ui/tooltip';
import { apiKey } from '../src/core/api';
import { api } from '../src/replica';
import { fakeServer, snapshotOf } from './fake';
import { contextOf, telemetryOf, processesOf } from './contextSeed';
import { stands } from './standingSeed';
import type { ChainView } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { cardRecord, chainRead, chainRecord } from './records';

let processSample = 0;
const params = new URLSearchParams(location.search);
const iso = (ms: number) => new Date(ms).toISOString();
const brief = `Make the billing worker retry a charge that Shopify answered with a 429, instead of marking the subscription failed.

- Read \`internal/billing/charge.go\` and \`internal/billing/worker.go\` first: the retry belongs where the answer is read.
- Keep the backoff the queue already uses (\`sqs.ChangeMessageVisibility\`); do not add a sleep in the handler.
- A charge retried more than five times is failed as today, with the last answer recorded.

${'The owner wants the failure mode visible in the worker logs, with the shop id and the attempt number on every line. '.repeat(4)}`;
const card = cardRecord({
  id: 'eng', home: 'ch', agent: 'engineer', name: 'Engineer', origin: 'hired', model: 'sonnet', mode: 'auto',
  cwd: '/home/owner/.remit/work/ch_5bddcf0823e3/subscriptionfns', status: params.get('processes') === 'stopped' ? 'stopped' : 'running', sessionId: '795811b8-a60a-4e42-b1cc-aa1d5a340445',
  brief, deliverable: 'A branch where a 429 from Shopify is retried with the queue backoff, with a test that fails before the change.',
  criteria: ['A 429 is retried, not failed', 'The sixth 429 fails the charge with the answer recorded', 'Every log line carries the shop id and the attempt'],
  plan: [], ctxUsed: 102_400, ctxSize: 1_000_000, turns: 14, queued: 0, createdAt: iso(Date.now() - 3_600_000), updatedAt: iso(Date.now()), rev: 1,
});
const view: ChainView = {
  chain: chainRead(chainRecord({ id: 'ch', title: 'Retry a rate-limited charge', status: 'running', chairId: 'chair', createdAt: iso(Date.now() - 3_600_000), updatedAt: iso(Date.now()) })),
  cards: [card], edges: [], retrospectives: [], decisions: [], helps: [], verification: { chainId: '', gates: [], judges: [], rev: 0 }, datadog: [], landings: [], drafts: [], workflows: [], agents: [],
  standing: stands('ch', 'Running', 'running'),
};
fakeServer({
  appSnapshot: () => snapshotOf({}),
  cardContext: async () => {
    if (params.get('slow')) await new Promise((r) => setTimeout(r, 2000));
    const counted = params.get('counted') === 'none' ? 'none' : params.get('counted') === 'stored' ? 'stored' : 'now';
    return contextOf(card, { counted, loaded: params.get('loaded') !== 'none', given: params.get('given') === 'none' ? null : undefined });
  },
  cardProcesses: async () => processesOf({
    stopped: params.get('processes') === 'stopped',
    shared: params.get('processes') === 'shared',
    cpu: 118.4 + (processSample++ % 5),
  }),
  cardTelemetry: async () => telemetryOf({ none: params.get('telemetry') === 'none' }),
});

const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/', component: { render: () => null } }] });
await router.push({ path: '/', query: params.get('panel') ? { panel: params.get('panel')! } : {} });
createApp({
  render: () =>
    h(TooltipProvider, { delayDuration: 300 }, () => h('div', { class: 'flex h-screen justify-end' }, [h(CardDrawer, { view, card })])),
})
  .use(router)
  .provide(apiKey, api)
  .mount('#app');
