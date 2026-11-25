// The Plugins page (ExtensionsView, the Integrations group's tab) in the whole app, on made-up data: a plugin that ships with Remit,
// one the owner applied, one he disabled, an API behind the proxy, an outside MCP server and one with a very long name and tool text. No server, no
// sign-in: a fake client answers the calls, through the same sync as the real app. Open /verify/extensions.html (?at=/plugins/<id>
// opens one). What the page asked is in window.calls; window.push(name, arg) is the server pushing a change (extensionUpdated,
// extensionDeleted).
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
import type { Actor, Extension, ExtensionToolInput, Snapshot } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { actorOf, chainRead } from './records';

const MIN = 60_000;
const iso = (ms: number) => new Date(ms).toISOString();
const OWNER: Actor = actorOf('owner', 'owner@example.com', 'web');
const calls: unknown[] = [];
Object.assign(window, { calls });

const tool = (name: string, what: string, effect = 'read', inputs: ExtensionToolInput[] = []) => ({ name, what, effect, inputs });
const input = (name: string, type: string, what: string, extra: Partial<ExtensionToolInput> = {}): ExtensionToolInput => ({ name, type, what, required: false, choices: [], ...extra });
const extensions = new Map<string, Extension>(
  [
    { id: 'amplitude', label: 'Amplitude', capability: 'customers', origin: 'bundled', enabled: true, poll: false, ui: false,
      tools: [tool('amplitude_events', 'List the event types the product sends to Amplitude, busiest first.'), tool('amplitude_funnel', 'How many users get through a sequence of 2 to 10 events in order.'), tool('amplitude_user_activity', 'What one user did: their profile and their latest events.')] },
    { id: 'github', label: 'GitHub', capability: 'customers', origin: 'bundled', enabled: true, poll: true, ui: false,
      tools: [tool('github_issues', 'List the issues of a repository, the most recently updated first, as a compact list.', 'read', [
        input('repo', 'string', 'the repository as owner/name, such as octo-org/app', { required: true }),
        input('state', 'string', 'open (default), closed or all', { choices: ['open', 'closed', 'all'], default: '"open"' }),
        input('labels', 'string', 'label names separated by commas; an issue must carry every one'),
        input('since', 'string', 'only issues updated since: RFC3339 or relative to now such as -24h or -7d'),
        input('pull_requests', 'boolean', 'list pull requests too; without it they are left out'),
        input('limit', 'integer', 'issues per page, 1 to 100; default 30', { default: '30' }),
      ]), tool('github_issue', 'Read one issue of a repository from its number, then its comments in order.', 'read', [input('repo', 'string', 'the repository as owner/name', { required: true }), input('number', 'integer', 'the issue\'s number', { required: true })])] },
    { id: 'gitlab', label: 'GitLab', capability: 'data', origin: 'bundled', enabled: true, poll: false, ui: false, tools: [] },
    { id: 'acme', label: 'Acme billing', capability: 'data', origin: 'program', enabled: true, poll: true, ui: true, sha256: '56c0b3182827f88bec5a80532d399b9d045042508a7dbe13db1af77d2949a922', appliedAt: iso(Date.now() - 3 * 60 * MIN),
      tools: [tool('acme_list_invoices', 'List the invoices of a customer.'), tool('acme_refund', 'Refund an invoice. It cannot be undone.', 'irreversible')] },
    { id: 'vnpay', label: 'VNPay', capability: 'customers', origin: 'program', enabled: false, poll: false, ui: false, sha256: 'ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12', appliedAt: iso(Date.now() - 48 * 60 * MIN),
      disabledBy: OWNER, disabledAt: iso(Date.now() - 20 * MIN), tools: [tool('vnpay_list_payments', 'List payments since a cursor.')] },
    { id: 'crm', label: 'Acme CRM', capability: 'customers', origin: 'api', enabled: true, poll: false, ui: false, appliedAt: iso(Date.now() - 5 * 60 * MIN), tools: [] },
    { id: 'linear', label: 'Linear', capability: 'customers', origin: 'mcp', enabled: true, poll: false, ui: false, appliedAt: iso(Date.now() - 90 * MIN),
      tools: [tool('linear_list_issues', 'List issues in the workspace, filtered by team, state or assignee.', 'read', [input('team', 'string', 'the team key, such as ENG')]), tool('linear_create_issue', 'Create an issue in a team.', 'irreversible', [input('team', 'string', 'the team key', { required: true }), input('title', 'string', 'the issue title', { required: true })])] },
    { id: 'verylong', label: 'A plugin with a very long name that keeps going until it has to wrap', capability: 'observe', origin: 'program', enabled: true, poll: false, ui: false, sha256: '0'.repeat(64),
      tools: [tool('verylong_tool_with_a_long_name_that_wraps_somewhere_in_the_middle', 'A long description. '.repeat(30))] },
  ].map((e) => [e.id, e as Extension] as const),
);
const listed = () => [...extensions.values()].sort((a, b) => Number(b.origin === 'bundled') - Number(a.origin === 'bundled') || a.id.localeCompare(b.id));

const rail = (name: string, enabled: boolean) => async (id: string) => {
  const e = extensions.get(id);
  if (!e) throw new Error(`no extension ${id}`);
  calls.push({ [name]: id });
  const { disabledBy: _by, disabledAt: _at, ...rest } = e;
  const next: Extension = enabled ? { ...rest, enabled: true } : { ...rest, enabled: false, disabledBy: OWNER, disabledAt: iso(Date.now()) };
  extensions.set(id, next);
  void send('extensionUpdated', next);
  return { ok: true } as never;
};
const drafts = fakeDrafts('extensions', (name, arg) => send(name as never, arg as never));
const send: typeof fake.push = (name, arg) => fake.push(name, structuredClone(arg));
const fake = fakeServer({
  draftSave: drafts.draftSave,
  draftDelete: drafts.draftDelete as never,
  appSnapshot: async () =>
    structuredClone(snapshotOf({
      objectives: [], projects: [], employees: [], mentors: [], mentorStandings: [],
      chains: hookChains.map((c) => chainRead(c)), inbox: [], playbooks: [], capabilities: [], standings: hookStandings,
      workload: { running: 1, capacity: 4, pressure: '', queued: 0, waiting: 0, nextCheckBack: iso(Date.now() + 30 * MIN), rev: 1 },
      insights: [], agents: [], skills: [], sources: [], events: [], eventSignals: [], sensors: [], clientKeys: [], extensions: listed(), feedback: [],
      assistant: { ...assistantView(Date.now(), assistantCard(Date.now(), { status: 'idle', queued: 0, activity: '' }), assistantStanding('ready')), drafts: drafts.live(ASSISTANT) },
    })),
  extensionDisable: rail('extensionDisable', false) as never,
  extensionEnable: rail('extensionEnable', true) as never,
});
signedIn();

const router = appRouter(createMemoryHistory());
const params = new URLSearchParams(location.search);
await router.push(params.get('at') ?? '/plugins');
createApp(Shell).use(router).provide(apiKey, api).mount('#app');
Object.assign(window, { router, push: send, lose: fake.lose });
