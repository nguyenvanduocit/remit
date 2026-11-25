// The Events list (EventsView: every event newest first, the event open beside it) in the whole app, on made-up data at any
// volume: ?rows=<n> events (default 64; 5, 500, 5000 for the sizes the list is measured at), ?empty=1 none. No server, no sign-in: a fake client answers the calls through the same sync as the real app. Open
// /verify/eventsMany.html (?at=/events/event:gitlab%235 opens an event). window.router is the router; window.paintMs is how long
// the page took from its data arriving to its first frame.
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
import { ASSISTANT, assistantCard, assistantStanding, assistantView } from './assistantSeed';
import { eventsSeed, listed, hookChains, hookStandings } from './eventsSeed';
import '../src/theme.css';
import { markPaint } from './paint';
import { chainRead } from './records';

const params = new URLSearchParams(location.search);
const MIN = 60_000;
const iso = (ms: number) => new Date(ms).toISOString();
const seeded = params.get('empty') ? [] : eventsSeed(Number(params.get('rows') ?? 64));
const byId = new Map(seeded.map((e) => [e.id, e]));

const fake = fakeServer({
  appSnapshot: async () =>
    structuredClone(snapshotOf({
      objectives: [], projects: [], employees: [], mentors: [], mentorStandings: [],
      chains: hookChains.map((c) => chainRead(c)), inbox: [], playbooks: [], capabilities: [], standings: hookStandings,
      workload: { running: 1, capacity: 4, pressure: '', queued: 1, waiting: 1, nextCheckBack: iso(Date.now() + 30 * MIN), rev: 1 },
      insights: [], agents: [], skills: [], events: seeded.map(listed), eventSignals: [], sensors: [], clientKeys: [], feedback: [],
      assistant: { ...assistantView(Date.now(), assistantCard(Date.now(), { status: 'idle', queued: 0, activity: '' }), assistantStanding('ready')), drafts: [] },
    })),
  eventGet: async (id) => {
    const e = byId.get(id);
    if (!e) throw new Error(`event ${id} not found`);
    return structuredClone(e);
  },
  draftSave: async (d) => ({ ...d, updatedAt: iso(Date.now()), rev: 1 }) as never,
});
signedIn();

const router = appRouter(createMemoryHistory());
await router.push(params.get('at') ?? '/events');
createApp(Shell).use(router).provide(apiKey, api).mount('#app');
markPaint();
Object.assign(window, { router, push: fake.push, [ASSISTANT]: undefined });
