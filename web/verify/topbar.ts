// The top bar (TopBar, with the workload pill and its hover card) on made-up data. No server, no sign-in: a fake server
// answers the calls of the page's real client core. Open /verify/topbar.html; ?idle=1 has nothing running, waiting
// or queued and nothing unread; ?at=<path> marks the entry of that page's area (the app's own routes, blank), e.g.
// ?at=/signals, ?at=/chains/c1/tree, ?at=/feedback, ?at=/client-keys; ?assistant=working|needs|failed puts the Assistant's button in that state;
// ?people=1 has the waiting chain wait on people's replies alone, so it holds no check-back (the server sends none).
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp, h } from 'vue';
import { createMemoryHistory, createRouter } from 'vue-router';
import TopBar from '../src/components/TopBar.vue';
import { TooltipProvider } from '../src/components/ui/tooltip';
import { apiKey } from '../src/core/api';
import { api } from '../src/replica';
import { blankRoutes } from './blankRoutes';
import { fakeServer, snapshotOf } from './fake';
import { feedbackSeed } from './feedbackSeed';
import type { Chain, ChainOrigin } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { chainRead, chainRecord } from './records';

const params = new URLSearchParams(location.search);
const idle = params.get('idle');
const people = params.get('people');
const now = new Date().toISOString();
const chain = (id: string, title: string, status: Chain['status'], origin: ChainOrigin): Chain =>
  chainRecord({ id, title, goal: 'g', status, origin, chairId: `${id}-chair`, workdir: '/', createdAt: now, updatedAt: now, rev: 1 });
const chains = idle
  ? []
  : [
      chain('c1', 'mca: save-ad-insights và các consumer phụ thuộc vào nó nên tên rất dài', 'running', 'proposed'),
      chain('c2', 'Fix the empty body {} bug', 'waiting', 'owner'),
      chain('c3', 'Tidy the schedule', 'queued', 'routine'),
    ];
const workload = {
  running: idle ? 0 : 1,
  capacity: 4,
  pressure: params.get('pressure') === '1' ? "the host's load is 9.4 on 8 cores" : '',
  queued: idle ? 0 : 1,
  waiting: idle ? 0 : 1,
  nextCheckBack: people ? undefined : new Date(Date.now() + 30 * 60_000).toISOString(),
  rev: 1,
};
const tones = { working: ['running', 'Working'], needs: ['needs', 'Needs you'], failed: ['failed', 'Needs you'] } as const;
const standing = tones[params.get('assistant') as keyof typeof tones];
const idleAssistant = snapshotOf({}).assistant;
if (!idleAssistant) throw new Error('the seed snapshot has no Assistant');
const assistant = standing ? { ...idleAssistant, agent: { ...idleAssistant.agent, tone: standing[0], label: standing[1] } } : idleAssistant;
fakeServer({ appSnapshot: () => snapshotOf({ chains: chains.map((c) => chainRead(c)), workload, assistant, feedback: idle ? [] : feedbackSeed(Date.now()) }) });

const router = createRouter({ history: createMemoryHistory(), routes: blankRoutes });
await router.push(params.get('at') ?? '/board');
createApp({ render: () => h(TooltipProvider, { delayDuration: 300 }, () => h(TopBar, { toReview: idle ? 0 : 2 })) })
  .use(router)
  .provide(apiKey, api)
  .mount('#app');
