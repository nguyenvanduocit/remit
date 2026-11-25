// The open chain's header (ChainPage): its title, status and the tab row, on made-up data. The tab row is the one with every
// tab it can show (Live, Agent tree, Landings, Verification, Overview), to resize the
// window narrow like a chain open next to the agent drawer. No server, no sign-in. Open /verify/chainhead.html; ?tabs=few
// leaves Landings and Verification out.
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
import '../src/theme.css';
import { cardRecord, chainRead, chainRecord, landingRead, landingRecord, repoRecord } from './records';

const few = new URLSearchParams(location.search).get('tabs') === 'few';
const iso = (ms: number) => new Date(ms).toISOString();
const repo = (name: string) => repoRecord({ repo: name, project: name, path: '/', branch: 'feat/x', base: 'main' });
const landing = (n: number, name: string) => landingRecord({ id: `ln${n}`, chainId: 'ch', repo: name, version: { number: 1, title: 'fix: shop owner' }, state: { kind: 'ready' }, createdAt: iso(Date.now()) });
const chain = chainRecord({
  id: 'ch', title: 'Import agents and skills', goal: 'Import agents and skills.', status: 'running', chairId: 'chair', workdir: '/',
  createdAt: iso(Date.now() - 3_000_000), updatedAt: iso(Date.now()), rev: 1, repos: few ? [] : [repo('g/remit'), repo('g/sales-engine')],
});
const card = cardRecord({ id: 'chair', home: 'ch', agent: 'chair', name: 'Chair', origin: 'chair', model: 'opus', mode: 'auto', cwd: '/', status: 'running', plan: [], ctxUsed: 0, ctxSize: 0, turns: 1, queued: 0, createdAt: iso(Date.now() - 3_000_000), updatedAt: iso(Date.now()), rev: 1 });

const standing = few ? stands('ch', 'Running', 'running') : stands('ch', 'Running', 'running', { verification: { label: 'Approved', tone: 'done' } });
fakeServer({
  appSnapshot: () => snapshotOf({ chains: [chainRead(chain)], standings: [standing] }),
  chainGet: () => ({ chain: chainRead(chain), cards: [card], edges: [], retrospectives: [], decisions: [], datadog: [], landings: few ? [] : [landingRead(landing(1, 'g/remit')), landingRead(landing(2, 'g/sales-engine'))], drafts: [], workflows: [], agents: [], helps: [], verification: { chainId: 'ch', gates: [], judges: [], rev: 1 }, standing }),
  chainEvents: () => ({ events: [], truncated: false }),
});
openChain('ch');

const router = createRouter({ history: createMemoryHistory(), routes });
await router.push({ name: 'chain', params: { id: 'ch', tab: 'tree' } });
createApp({ render: () => h(TooltipProvider, { delayDuration: 300 }, () => h('div', { class: 'flex h-screen flex-col' }, [opened.view && h(ChainPage, { view: opened.view })])) })
  .use(router)
  .provide(apiKey, api)
  .mount('#app');
