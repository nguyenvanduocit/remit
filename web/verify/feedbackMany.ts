// The Feedback board (FeedbackView: one column per kind side by side, the feedback open beside one column) in the whole app, on
// made-up data at any volume: the seeded four (feedbackSeed) and ?rows=<n> more of every kind, one in three unread (0 by
// default; 500, 5000 for the sizes the board is measured at); ?empty=1 none. No server, no sign-in: a fake client answers the
// calls through the same sync as the real app. Open /verify/feedbackMany.html (?at=/feedback/fb_1 opens one). Opening one
// reads it (feedbackRead) and the server pushes it back read. window.router is the router; window.paintMs is how long the
// page took from its data arriving to its first frame.
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
import { feedbackSeed } from './feedbackSeed';
import { hookChains, hookStandings } from './eventsSeed';
import type { Feedback, FeedbackKind, Snapshot } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { markPaint } from './paint';
import { chainRead, feedbackRecord } from './records';

const params = new URLSearchParams(location.search);
const MIN = 60_000;
const iso = (ms: number) => new Date(ms).toISOString();
const now = Date.now();
const KINDS: FeedbackKind[] = ['practice', 'difficulty', 'complaint', 'suggestion'];
const NAMES = ['Engineer', 'Reviewer', 'Architect', 'QA', 'Researcher'];
const TEXTS = [
  'The brief names no repository, so I spent my first turns finding which one holds the billing code.',
  'Let the reviewer see the engineer’s test run, not only the diff: I re-ran the suite twice to learn what was known.',
  'Being sent back three times for the same missing test, each time with a different wording, is hard to act on.',
  'I answer from what the tools show, and I often cannot tell which chain the owner means by "this one".',
];
const more: Feedback[] = Array.from({ length: Number(params.get('rows') ?? 0) }, (_, i) => feedbackRecord({
  id: `fb_m${i}`, kind: KINDS[i % 4], name: NAMES[i % 5], text: `${TEXTS[i % 4]} (${i})`, home: i % 7 ? 'ch_deploy' : 'as_1', cardId: 'eng', agent: NAMES[i % 5].toLowerCase(), model: 'claude-sonnet-5-5',
  createdAt: iso(now - (30 + i * 11) * MIN), rev: 1, ...(i % 3 ? { readAt: iso(now - i * MIN) } : {}),
}));
const seeded = feedbackSeed(now).map((f) => ({ ...f, home: f.home === 'busy' ? 'ch_deploy' : f.home }));
const feedback: Feedback[] = params.get('empty') ? [] : [...seeded, ...more];

const fake = fakeServer({
  appSnapshot: async () =>
    structuredClone(snapshotOf({
      objectives: [], projects: [], employees: [], mentors: [], mentorStandings: [],
      chains: hookChains.map((c) => chainRead(c)), inbox: [], playbooks: [], capabilities: [], standings: hookStandings,
      workload: { running: 1, capacity: 4, pressure: '', queued: 1, waiting: 1, nextCheckBack: iso(now + 30 * MIN), rev: 1 },
      insights: [], agents: [], skills: [], events: [], eventSignals: [], sensors: [], clientKeys: [], feedback,
      assistant: { ...assistantView(now, assistantCard(now, { status: 'idle', queued: 0, activity: '' }), assistantStanding('ready')), drafts: [] },
    })),
  feedbackRead: async (id) => {
    const f = feedback.find((x) => x.id === id);
    if (!f) throw new Error(`feedback ${id} not found`);
    const read = { ...f, readAt: iso(Date.now()), rev: f.rev + 1 };
    setTimeout(() => void fake.push('feedbackUpdated', structuredClone(read)), 50);
    return structuredClone(read);
  },
  draftSave: async (d) => ({ ...d, updatedAt: iso(Date.now()), rev: 1 }) as never,
});
signedIn();

const router = appRouter(createMemoryHistory());
await router.push(params.get('at') ?? '/feedback');
createApp(Shell).use(router).provide(apiKey, api).mount('#app');
markPaint();
Object.assign(window, { router, push: fake.push });
