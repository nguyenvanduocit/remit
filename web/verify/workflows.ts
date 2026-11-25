// A chain's page (ChainPage) with the drawer of the picked card (CardDrawer), on a chain at work through scripts and subagents
// (workflowSeed): the Workflows tab lists its runs, the Agent tree shows each spawned card under the card it runs inside. Open
// /verify/workflows.html?tab=workflows (or tree), &card=<id> to open a card in the drawer, e.g. sub-idem, a subagent that takes
// no message. Stop on a run answers as the server would, pushing the run stopped. No server, no sign-in; window.calls lists
// what the page asked.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { computed, createApp, h } from 'vue';
import { createMemoryHistory, createRouter } from 'vue-router';
import ChainPage from '../src/components/ChainPage.vue';
import CardDrawer from '../src/components/CardDrawer.vue';
import { TooltipProvider } from '../src/components/ui/tooltip';
import { apiKey } from '../src/core/api';
import { api } from '../src/replica';
import { chairOf, opened, openChain } from '../src/openChain';
import { routes } from '../src/router';
import { fakeServer, snapshotOf } from './fake';
import { stands } from './standingSeed';
import { workflowSeed } from './workflowSeed';
import '../src/theme.css';
import { actorOf, chainRead, decisionRead, decisionRecord } from './records';

const params = new URLSearchParams(location.search);
const { chain, cards, agents, workflows, employees } = workflowSeed(Date.now());
const calls: string[] = [];
Object.assign(window, { calls });
const fake = fakeServer({
  appSnapshot: () => snapshotOf({ chains: [chainRead(chain)], employees, standings: [stands('ch', 'Running', 'running', { working: true })] }),
  chainGet: async () => ({ chain: chainRead(chain), cards, edges: [], retrospectives: [], decisions: [decisionRead(decisionRecord({ id: 'd_wf', chainId: 'ch', kind: 'workflow', by: actorOf('agent', 'chair'), text: 'Run workflow review-fanout (w_fan).', why: 'Five reviews of the retry change at once, then one verdict: the shape is known, so the run drives it.', cardId: 'chair', createdAt: new Date(Date.now() - 30 * 60_000).toISOString() }), false)], helps: [], verification: { chainId: 'ch', gates: [], judges: [], rev: 0 }, datadog: [], landings: [], drafts: [], workflows, agents, standing: stands('ch', 'Running', 'running', { working: true }) }),
  chainEvents: async () => ({ events: [], truncated: false }),
  cardEvents: async () => ({ events: [], truncated: false }),
  workflowStop: async (id, why) => {
    calls.push(`workflowStop ${id} ${JSON.stringify(why)}`);
    const w = opened.view?.workflows.find((x) => x.id === id);
    if (w) fake.push('workflowUpdated', { ...w, status: 'stopped', error: 'stopped by the owner', endedAt: new Date().toISOString(), steps: w.steps.map((s) => (s.status === 'pending' ? { ...s, status: 'failed', error: 'the run was stopped' } : s)), rev: w.rev + 1 });
    return {} as never;
  },
  cardStop: async (id) => (calls.push(`cardStop ${id}`), cards.find((c) => c.id === id)!),
});
openChain('ch');

const router = createRouter({ history: createMemoryHistory(), routes });
await router.push({ name: 'chain', params: { id: 'ch', tab: params.get('tab') ?? 'workflows' }, query: params.get('card') ? { card: params.get('card')! } : {} });
const picked = computed(() => {
  const v = opened.view;
  const id = router.currentRoute.value.query.card;
  return v && ((typeof id === 'string' && v.cards.find((c) => c.id === id)) || chairOf(v));
});
createApp({
  render: () => h(TooltipProvider, { delayDuration: 300 }, () => h('div', { class: 'flex h-screen' }, [
    h('main', { class: 'flex min-w-0 flex-1 flex-col' }, [opened.view && h(ChainPage, { view: opened.view })]),
    h('div', { class: 'flex w-[480px] flex-none border-l border-line' }, [opened.view && picked.value && h(CardDrawer, { view: opened.view, card: picked.value })]),
  ])),
})
  .use(router)
  .provide(apiKey, api)
  .mount('#app');
