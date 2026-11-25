// The Crew boards (CrewPage: Playbooks, Agents, Skills and Tools, each one column per group side by side, the open one
// beside its column) in the whole app, on made-up data at any volume: the library seed (librarySeed, with playbooks of both
// kinds) and ?rows=<n> more of every kind, half built in or imported (0 by default; 500, 5000 for the sizes the boards are
// measured at); ?empty=1 none of any kind. No server, no sign-in: a fake client answers the calls through the same sync as the
// real app. Open /verify/crewMany.html?at=/playbooks (or /agents, /skills, /tools; ?at=/agents/engineer opens one).
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
import { capabilities } from './contextSeed';
import { agentsSeed, changesSeed, credentialItems, skillFileSeed, skillsSeed, toolsSeed } from './librarySeed';
import type { Agent, Playbook, ListedSkill, Snapshot, ToolSource } from '../src/core/generated/types.generated';
import '../src/theme.css';
import { markPaint } from './paint';
import { agentRecord, employeeRead, employeeRecord, playbookRecord, seedRecord } from './records';

const params = new URLSearchParams(location.search);
const MIN = 60_000;
const iso = (ms: number) => new Date(ms).toISOString();
const now = Date.now();
const rows = Number(params.get('rows') ?? 0);
const empty = !!params.get('empty');

const playbook = (id: string, name: string, extra: Partial<Playbook> = {}): Playbook => playbookRecord({
  id, name, description: `${name}: how the Chair usually goes about it, and what it must never do on its own.`, group: 'engineering', rev: 1,
  brief: 'Hire freely. Ask the owner only what cannot be found.\n\n- Research first, then one engineer.\n- Every change has a test.',
  seeds: [seedRecord({ agent: 'research', name: 'Analyst', brief: 'Find where it breaks.' }), seedRecord({ agent: 'engineer', name: 'Engineer', brief: 'Fix it, with a test first.' }), seedRecord({ agent: 'qc', name: 'QC', brief: 'Verify on the real system.' })],
  ...extra,
});
const playbooks: Playbook[] = empty ? [] : [
  playbook('bugfix', 'Fix a bug', { builtin: true }),
  playbook('feature', 'Build a feature', { builtin: true }),
  playbook('backfill', 'Backfill / migration', { group: 'data', builtin: true, edited: true }),
  playbook('audit', 'Dependency audit of the API', { group: 'product', seeds: [] }),
  playbook('sweep', 'Nightly sweep of stale branches, with a very long name that has to be cut on its card and said whole in a hint', { group: 'general' }),
  ...Array.from({ length: rows }, (_, i) => playbook(`pb_m${i}`, `Playbook ${i}: ${['triage an alert', 'refresh the docs', 'rotate a key', 'review a dependency'][i % 4]}`, { group: ['engineering', 'data', 'product', 'general'][i % 4] ?? 'general', ...(i % 2 ? { builtin: true, ...(i % 6 === 1 ? { edited: true } : {}) } : {}) })),
];
const agents: Agent[] = empty ? [] : [
  ...agentsSeed(),
  ...Array.from({ length: rows }, (_, i): Agent => agentRecord({
    id: `ag_m${i}`, name: `Agent ${i} ${['Reader', 'Writer', 'Checker', 'Planner'][i % 4]}`, description: `Does one thing well (${i}): ${['reads and never writes', 'writes with a test first', 'checks on the real system', 'plans and hires'][i % 4]}.`,
    group: ['engineering', 'data', 'verification', 'general'][i % 4] ?? 'general', loadout: { instructions: 'Say what you could not confirm.', tools: i % 3 ? ['Read', 'Grep'] : null }, rev: 1, ...(i % 2 ? { builtin: true } : {}), ...(i % 5 === 0 ? { model: 'opus' } : {}), ...(i % 7 === 0 ? { signs: 'review' as const } : {}),
  })),
];
const skills: ListedSkill[] = empty ? [] : [
  ...skillsSeed(now),
  ...Array.from({ length: rows }, (_, i): ListedSkill => ({
    name: `skill-${i}`, description: `When to use it (${i}): ${['querying the warehouse', 'writing release notes', 'reviewing a migration', 'reading the vault'][i % 4]}.`,
    group: ['engineering', 'data', 'design', 'general'][i % 4] ?? 'general', files: [{ path: 'SKILL.md', size: 400 + i }], rev: 1, ...(i % 2 ? { source: { kind: 'claude-code', name: `skill-${i}`, hash: 'c0ffee', at: iso(now - (60 + i) * MIN) } } : {}),
  })),
];
const sources: ToolSource[] = empty ? [] : [
  ...toolsSeed(),
  ...Array.from({ length: rows }, (_, i): ToolSource => ({
    id: `src_m${i}`, label: `Source ${i}`, capability: ['data', 'customers', 'observe'][i % 3],
    tools: [{ name: `source_${i}_read`, what: 'Read it.' }],
    connections: [{ name: 'default', description: '', configured: i % 2 === 0, missing: i % 2 ? ['TOKEN'] : [], problem: '', variables: [{ name: 'TOKEN', what: 'a token that may only read', secret: true, optional: false, address: false, state: i % 2 ? 'unset' : 'environment', value: '' }] }],
  })),
];
const changeHistory = changesSeed(now);

const fake = fakeServer({
  appSnapshot: async () =>
    structuredClone(snapshotOf({
      objectives: [], projects: [], employees: empty ? [] : [employeeRead(employeeRecord({ id: 'e_pay', number: 7, label: 'Payments Chair', template: 'chair', templateRev: 1, title: 'chair', reportsTo: 'owner', hiredBy: 'owner', status: 'active', createdAt: iso(now) }), { shown: 'Payments Chair · E-7 (chair)', tag: 'E-7 (chair)', face: { shape: 'square', hue: 210, accessory: 'visor' } })], mentors: [], mentorStandings: [],
      chains: [], inbox: credentialItems(now), playbooks, capabilities, standings: [],
      workload: { running: 0, capacity: 4, pressure: '', queued: 0, waiting: 0, rev: 1 },
      insights: [], agents, skills, sources, events: [], eventSignals: [], sensors: [], clientKeys: [], feedback: [],
      assistant: { ...assistantView(now, assistantCard(now, { status: 'idle', queued: 0, activity: '' }), assistantStanding('ready')), drafts: [] },
    })),
  changesGet: async (kind, name) => changeHistory[`${kind}/${name}`] ?? [],
  skillFile: skillFileSeed(now),
  draftSave: async (d) => ({ ...d, updatedAt: iso(Date.now()), rev: 1 }) as never,
});
signedIn();

const router = appRouter(createMemoryHistory());
await router.push(params.get('at') ?? '/playbooks');
createApp(Shell).use(router).provide(apiKey, api).mount('#app');
markPaint();
Object.assign(window, { router, push: fake.push });
