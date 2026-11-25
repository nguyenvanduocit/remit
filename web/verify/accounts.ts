// The Accounts page (AccountsView, in Integrations) in the whole app, on made-up data: the machine's own login, a login the
// owner added that hit its limit and says when it resets, one that takes work with its weekly limit near, and two accounts plugins provide, one that takes work and one with
// long names that is out with a long error. No server, no sign-in: a fake client answers the calls, through the same sync as
// the real app. Open /verify/accounts.html. Add account makes the sign-in item appear on the page with its link; the code
// `good` signs it in and the account joins the list as work@example.com, any other code is refused. ?at=/accounts/<id> opens
// an account; ?none=1 makes every account out. What the page asked is in window.calls.
import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import { createApp } from 'vue';
import { createMemoryHistory } from 'vue-router';
import Shell from '../src/components/Shell.vue';
import { apiKey } from '../src/core/api';
import { api } from '../src/replica';
import { accountLabel } from '../src/accounts';
import { signedIn } from '../src/session';
import { appRouter } from '../src/router';
import { fakeServer, snapshotOf } from './fake';
import { fakeDrafts } from './fakeDrafts';
import { ASSISTANT, assistantCard, assistantStanding, assistantView } from './assistantSeed';
import { hookChains, hookStandings } from './eventsSeed';
import type { AccountHealth, Extension, Health, InboxItem, Snapshot } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { accountHealth, chainRead, inboxRecord } from './records';

const params = new URLSearchParams(location.search);
const iso = (ms: number) => new Date(ms).toISOString();
const calls: unknown[] = [];
Object.assign(window, { calls });

const none = !!params.get('none');
const GATEWAY = 'company-llm-gateway-for-the-platform-team';
const SHARED = 'production-eu-central-1-shared-with-data-science';
let accounts: AccountHealth[] = [
  accountHealth({ id: 'default', kind: 'machine', email: 'owner@example.com', ok: !none, ...(none ? { error: "You've hit your weekly limit · resets Nov 4 at 1pm" } : {}) }),
  accountHealth({ id: 'a_9c8b7a', kind: 'login', email: 'team@example.com', ok: false, error: "You've hit your session limit · resets 9:40am (UTC)", resetsAt: iso(Date.now() + (2 * 60 + 15) * 60_000) }),
  accountHealth({ id: 'a_4d5e6f', kind: 'login', email: 'design@example.com', ok: true, nearLimit: 'Weekly limit', utilization: 0.92, resetsAt: iso(Date.now() + (3 * 24 + 5) * 3_600_000) }),
  accountHealth({ id: 'bedrock.us-east-1', kind: 'plugin', plugin: 'bedrock', connection: 'us-east-1', ok: !none, ...(none ? { error: 'ThrottlingException: Too many requests, please wait before trying again' } : {}) }),
  accountHealth({ id: `${GATEWAY}.${SHARED}`, kind: 'plugin', plugin: GATEWAY, connection: SHARED, ok: false,
    error: 'API Error: 401 {"type":"error","error":{"type":"authentication_error","message":"The gateway refused the token: it expired at 2026-11-10T08:00:00Z and the connection has no refresh credential"}} · Fix external API key' }),
];
// The plugins those accounts come from, as the Plugins page lists them.
const extensions: Extension[] = [
  { id: 'bedrock', label: 'Amazon Bedrock', capability: 'accounts', origin: 'program', enabled: true, poll: false, ui: false, tools: [], sha256: 'ab12cd34'.repeat(8), appliedAt: iso(Date.now() - 3 * 3_600_000) },
  { id: GATEWAY, label: 'Company LLM gateway', capability: 'accounts', origin: 'program', enabled: true, poll: false, ui: false, tools: [], sha256: '56c0b318'.repeat(8), appliedAt: iso(Date.now() - 26 * 3_600_000) },
];
let rev = 1;
const health = (): Health => ({
  ok: accounts.some((a) => a.ok),
  error: '',
  ...(accounts.some((a) => a.ok) ? {} : { error: accounts.map((a) => `${accountLabel(a)}: ${a.error}`).join('; ') }),
  accounts: structuredClone(accounts), checkedAt: iso(Date.now()), unreadable: 0, rev: rev++,
});
const login = (): InboxItem => inboxRecord({
  id: 'i_login', home: 'assistant', cardId: 'c_assistant', kind: 'login', title: 'Add a Claude account', status: 'open', createdAt: iso(Date.now()), account: 'a_new',
  detail: "Open the sign-in page, sign in to the Claude account to add, and type the code the page shows into the field of this item. If your browser runs on the server's machine, signing in finishes by itself. The code goes to the sign-in and nowhere else: no agent sees it.",
  link: 'https://claude.com/cai/oauth/authorize?code=true&client_id=example&state=abc',
});
let item: InboxItem | null = null;

const drafts = fakeDrafts('accounts', (name, arg) => send(name as never, arg as never));
const send: typeof fake.push = (name, arg) => fake.push(name, structuredClone(arg));
const fake = fakeServer({
  draftSave: drafts.draftSave,
  draftDelete: drafts.draftDelete as never,
  appSnapshot: async () =>
    structuredClone(snapshotOf({
      objectives: [], projects: [], employees: [], mentors: [], mentorStandings: [], chains: hookChains.map((c) => chainRead(c)), inbox: [], playbooks: [], capabilities: [], standings: hookStandings, health: health(),
      workload: { running: 0, capacity: 4, pressure: '', queued: 0, waiting: 0, nextCheckBack: iso(Date.now() + 30 * 60_000), rev: 1 },
      insights: [], agents: [], skills: [], sources: [], events: [], eventSignals: [], sensors: [], clientKeys: [], extensions, feedback: [],
      assistant: { ...assistantView(Date.now(), assistantCard(Date.now(), { status: 'idle', queued: 0, activity: '' }), assistantStanding('ready')), drafts: drafts.live(ASSISTANT) },
    })),
  accountAdd: async () => {
    calls.push('accountAdd');
    item = login();
    void send('inboxUpdated', item);
    return item;
  },
  inboxAnswer: async (id: string, _option: string, text: string) => {
    calls.push({ inboxAnswer: id, text });
    if (text !== 'good') throw new Error('The sign-in refused the code');
    if (item) void send('inboxUpdated', { ...item, status: 'answered', answer: 'signed in as work@example.com', rev: item.rev + 1 });
    // As the server lists them: the logins, oldest first, then the plugins' accounts.
    accounts = [...accounts.filter((a) => a.kind !== 'plugin'), accountHealth({ id: 'a_1f2e3d', kind: 'login', email: 'work@example.com', ok: true }), ...accounts.filter((a) => a.kind === 'plugin')];
    void send('healthUpdated', health());
    return {};
  },
  accountRemove: async (id: string) => {
    calls.push({ accountRemove: id });
    accounts = accounts.filter((a) => a.id !== id);
    void send('healthUpdated', health());
    return {};
  },
  healthCheck: async () => {
    calls.push('healthCheck');
    return health();
  },
});
signedIn();

const router = appRouter(createMemoryHistory());
await router.push(params.get('at') ?? '/accounts');
createApp(Shell).use(router).provide(apiKey, api).mount('#app');
Object.assign(window, { router, push: send });
