// The Client keys page (ClientKeysView, the Integrations group's fourth tab) in the whole app, on made-up data: a key in use that started
// chains, a paused one, one never used with no playbook yet, and one with a very long name and note. No server, no sign-in: a
// fake client answers the calls, through the same sync as the real app. Open /verify/clientkeys.html (?at=/client-keys/<id> opens
// a key; ?keyfail=1 makes the server unable to read a value back; ?empty=1 has no key). What the page asked is in window.calls;
// window.push(name, arg) is the server pushing a change (clientKeyUpdated, clientKeyDeleted: the owner told the Assistant).
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
import { fakeDrafts } from './fakeDrafts';
import { ASSISTANT, assistantCard, assistantStanding, assistantView } from './assistantSeed';
import { hookChains, hookStandings } from './eventsSeed';
import type { Actor, Chain, ClientKey, Playbook, Snapshot } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { actorOf, chainRead, clientKeyRecord, playbookRecord } from './records';

const params = new URLSearchParams(location.search);
const MIN = 60_000;
const HOUR = 60 * MIN;
const iso = (ms: number) => new Date(ms).toISOString();
const OWNER: Actor = actorOf('owner', 'owner@example.com', 'web');
const ASSISTANT_ACTOR: Actor = actorOf('assistant', 'c_assistant');
const calls: unknown[] = [];
Object.assign(window, { calls });
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

const playbooks: Playbook[] = [
  playbookRecord({ id: 'research', name: 'Read-only research', group: 'engineering' }),
  playbookRecord({ id: 'prd-stories', name: 'PRD → Stories', group: 'engineering' }),
];
const key = (id: string, name: string, madeAgo: number, extra: Partial<ClientKey> = {}): ClientKey => clientKeyRecord({
  id, name, by: ASSISTANT_ACTOR, chainsPerDay: 3, actionsPerHour: 60, createdAt: iso(Date.now() - madeAgo), updatedAt: iso(Date.now() - madeAgo), rev: 1, ...extra,
});
const keys = new Map<string, ClientKey>(
  params.get('empty')
    ? []
    : [
        key('ck_doctor', 'doctor', 20 * MIN, { note: 'The doctor that evaluates Remit every three hours', lastUsedAt: iso(Date.now() - 4 * MIN), lastAddr: '127.0.0.1', rev: 2 }),
        key('ck_old', 'spike-runner', 5 * 24 * HOUR, { paused: true, rev: 3, by: OWNER, updatedAt: iso(Date.now() - 2 * 24 * HOUR), lastUsedAt: iso(Date.now() - 2 * 24 * HOUR), lastAddr: '127.0.0.1' }),
        key('ck_new', 'never-used', 3 * MIN, { chainsPerDay: 0 }),
        key('ck_long', 'an-evaluator-with-a-very-long-name-that-keeps-going-and-going-until-it-has-to-wrap', 9 * 24 * HOUR, { note: 'A long note. '.repeat(30) }),
      ].map((k) => [k.id, k] as const),
);
const listed = () => [...keys.values()].sort((a, b) => b.createdAt.localeCompare(a.createdAt));
// The chains the doctor started: marked as a client's, as the server marks them.
const started = (id: string, title: string, status: Chain['status'], agoMin: number): Chain =>
  ({ ...hookChains[0], id, title, status, origin: 'client', by: actorOf('client', 'ck_doctor', 'mcp'), createdAt: iso(Date.now() - agoMin * MIN), updatedAt: iso(Date.now() - agoMin * MIN), rev: 1 });
const chains = [...hookChains, started('ch_probe1', 'Explain TestRunRetry', 'done', 90), started('ch_probe2', 'Find callers of ensureChair', 'running', 12)];

const rail = (name: string, paused: boolean) => async (id: string) => {
  const k = keys.get(id);
  if (!k) throw new Error(`no client key ${id}`);
  calls.push({ [name]: id });
  const next: ClientKey = { ...k, paused, by: OWNER, updatedAt: iso(Date.now()), rev: k.rev + 1 };
  keys.set(id, next);
  void send('clientKeyUpdated', next);
  return { ok: true } as never;
};
const drafts = fakeDrafts('clientkeys', (name, arg) => send(name as never, arg as never));
const send: typeof fake.push = (name, arg) => fake.push(name, structuredClone(arg));
const fake = fakeServer({
  draftSave: drafts.draftSave,
  draftDelete: drafts.draftDelete as never,
  appSnapshot: async () =>
    structuredClone(snapshotOf({
      objectives: [], projects: [], employees: [], mentors: [], mentorStandings: [],
      chains: chains.map((c) => chainRead(c)), inbox: [], playbooks, capabilities: [], standings: hookStandings,
      workload: { running: 1, capacity: 4, pressure: '', queued: 0, waiting: 0, nextCheckBack: iso(Date.now() + 30 * MIN), rev: 1 },
      insights: [], agents: [], skills: [], sources: [], events: [], eventSignals: [], sensors: [], clientKeys: listed(),
      assistant: { ...assistantView(Date.now(), assistantCard(Date.now(), { status: 'idle', queued: 0, activity: '' }), assistantStanding('ready')), drafts: drafts.live(ASSISTANT) },
    })),
  // A value is read from the server a moment later; the server may be unable to (it was sealed under a key it lost).
  clientKey: async (id) => {
    calls.push({ clientKey: id });
    await wait(350);
    if (params.get('keyfail') || !keys.has(id)) throw new Error('the value of this client key cannot be read back: ask the Assistant to rotate it');
    return { key: `ck_${id.slice(3)}_9f2c1d7a5b8e4036a1d2c3b4e5f60718293a4b5c6d7e` };
  },
  clientKeyPause: rail('clientKeyPause', true) as never,
  clientKeyResume: rail('clientKeyResume', false) as never,
});
signedIn();

const router = appRouter(createMemoryHistory());
await router.push(params.get('at') ?? '/client-keys');
createApp(Shell).use(router).provide(apiKey, api).mount('#app');
Object.assign(window, { router, push: send, lose: fake.lose });
