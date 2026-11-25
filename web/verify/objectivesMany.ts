// The Objectives page (ObjectivesView) in the whole app, on made-up data (objectivesSeed): objectives of every state, key results of
// every status, a very long outcome and a retired owner, three of them on projects (projectsSeed: ?project=p_storefront narrows to one). ?n=<count> adds that many objectives (500 for the volume the page is
// measured at); ?empty=1 has none; No server, no sign-in: a fake client answers the calls
// through the same sync as the real app. Open /verify/objectivesMany.html?at=/objectives (or ?at=/objectives/ob_refunds).
// window.router is the router; window.paintMs is how long the page took from its data arriving to its first frame.
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
import { assistantCard, assistantStanding, assistantView } from './assistantSeed';
import { employeesSeed, many, objectivesSeed, standingsOf } from './objectivesSeed';
import { projectsSeed } from './projectsSeed';
import '../src/theme.css';
import { markPaint } from './paint';
import { chainRead } from './records';

const params = new URLSearchParams(location.search);
const now = Date.now();
const n = Number(params.get('n') ?? 0);
const seed = params.get('empty') ? { views: [], chains: [] } : objectivesSeed(now);
const extra = many(n, now);
const views = [...seed.views, ...extra.views];
const chains = [...seed.chains, ...extra.chains];

const fake = fakeServer({
  appSnapshot: async () =>
    structuredClone(snapshotOf({
      objectives: views, projects: projectsSeed(now), employees: employeesSeed(now), mentors: [], mentorStandings: [], chains: chains.map((c) => chainRead(c)), standings: standingsOf(chains), inbox: [], playbooks: [], capabilities: [],
      workload: { running: 0, capacity: 4, pressure: '', queued: 0, waiting: 0, rev: 1 },
      insights: [], agents: [], skills: [], sources: [], events: [], eventSignals: [], sensors: [], clientKeys: [], feedback: [],
      assistant: { ...assistantView(now, assistantCard(now, { status: 'idle', queued: 0, activity: '' }), assistantStanding('ready')), drafts: [] },
    })),
  draftSave: async (d) => ({ ...d, updatedAt: new Date().toISOString(), rev: 1 }) as never,
  changesGet: async () => [],
});
signedIn();

const router = appRouter(createMemoryHistory());
await router.push(params.get('at') ?? '/objectives');
createApp(Shell).use(router).provide(apiKey, api).mount('#app');
markPaint();
Object.assign(window, { router, push: fake.push });
