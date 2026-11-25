// What the Agents, Skills and Tools pages list on made-up data, the credential requests waiting on the owner, and what a card
// of one of these agents was given. Awkward cases on purpose: a built-in agent the owner edited, one that signs a gate, one
// with no tools, long descriptions, skills of one file and of many (one with a file long enough to scroll, one named with a
// dot), and sources of every standing: one the environment serves, one with a staging and a production connection, one whose
// stored secret cannot be opened, one with nothing connected, one with a long address and one with many variables.
import type { Actor, Agent, Change, ContextGiven, InboxItem, ListedSkill, ConnectionView, SourceVariable, ToolSource } from '../src/core/generated/types.generated';
import { actorOf, agentRecord, changeRecord, inboxRecord, optionOf } from './records';

const MIN = 60_000;
const at = (now: number, ago: number) => new Date(now - ago * MIN).toISOString();
const GATE_TOOLS = ['Read', 'Grep', 'Glob', 'Bash'];

// The server lists agents by id.
export const agentsSeed = (): Agent[] => [
  agentRecord({ id: 'db-reader', group: 'data', name: 'Database reader', description: 'Reads the production database and answers with numbers; never writes.', model: 'opus', loadout: { instructions: 'Use the `starrocks_query` tool only. A query that is not a `SELECT` is refused by the tool; do not look for a way around it.\n\nAnswer with the query you ran and its numbers.', tools: ['Read', 'Grep'], skills: [] }, rev: 3, edited: true }),
  agentRecord({ id: 'docs', group: 'product', name: 'Docs writer', description: 'Writes and fixes documentation from the code it reads, in the owner’s language, and says what it could not confirm.', loadout: { instructions: 'Write for a reader who has not seen the code.\n\n- Read the code before you write a line about it.\n- Say what you could not confirm.\n- Keep each page to one subject.', tools: ['Read', 'Edit', 'Write', 'Grep', 'Glob'], skills: ['vault.help'] }, rev: 1 }),
  agentRecord({ id: 'engineer', group: 'engineering', name: 'Engineer', description: 'Writes the change with a test first, in the repository’s own style, and opens nothing for review itself.', model: 'sonnet', loadout: { instructions: '# Engineer\n\nWrite the failing test first, then the change that makes it pass.\n\n1. Read the files the goal names, then the callers of what you will change.\n2. Run the suite for the scope you changed before you report.\n3. Report with the command and its last lines, not "done".\n\n> Never push, merge or close a merge request: the Chair does.', tools: ['Read', 'Edit', 'Write', 'Grep', 'Glob', 'Bash', 'Skill'], skills: ['test-driven-development', 'pinia'] }, rev: 4 }),
  agentRecord({ id: 'frontend', group: 'engineering', name: 'Frontend', description: 'Builds Vue screens with the shadcn components and looks at each one in a real browser before it reports.', model: 'opus', loadout: { instructions: 'Look at what you build: serve the verify pages, screenshot them, read the PNG, measure with `getBoundingClientRect()`.', tools: null, skills: ['vue', 'pinia', 'shadcn-vue', 'frontend-design', 'vite'] }, rev: 2 }),
  agentRecord({ id: 'qc', group: 'verification', name: 'QC', description: 'Checks the result on the real system and gives a verdict; never edits.', signs: 'qc', builtin: true, edited: true, loadout: { instructions: 'Run the thing. A claim you did not check is a hypothesis; say so.\n\nGive the verdict with the evidence: the command you ran and what it printed.', tools: [...GATE_TOOLS, 'WebFetch'], skills: [] }, rev: 5 }),
  agentRecord({ id: 'release', group: 'product', name: 'Release', description: 'Cuts a release from a green main and writes its notes.', model: 'haiku', loadout: { instructions: 'Tag only what CI has passed. Write the notes for the people who use it.', tools: ['Read', 'Bash', 'Grep', 'Skill'], skills: ['release-notes'] }, rev: 1 }),
  agentRecord({ id: 'researcher', group: 'engineering', name: 'Researcher', description: 'Finds where a thing breaks and reports what it found; changes nothing.', loadout: { instructions: 'Report the file and line for every claim, and how sure you are.', tools: ['Read', 'Grep', 'Glob', 'WebSearch', 'WebFetch'], skills: [] }, rev: 1 }),
  agentRecord({ id: 'reviewer', group: 'verification', name: 'Reviewer', description: 'Reads a change as a maintainer would and gives a verdict; never edits.', signs: 'review', builtin: true, loadout: { instructions: 'Read the diff and the code around it. Name what is wrong, where, and why; approve only what you would maintain.', tools: GATE_TOOLS, skills: [] }, rev: 1 }),
  agentRecord({ id: 'security', group: 'verification', name: 'Security', description: 'Reviews a change for what an attacker could do with it.', loadout: { instructions: '', tools: ['Read', 'Grep', 'Glob'], skills: ['security-review'] }, rev: 2 }),
  agentRecord({ id: 'talker', group: 'general', name: 'Talker', description: 'Answers questions from what it is told, with no tool at all.', loadout: { instructions: 'You have no tools. Answer from what you were given, and say when that is not enough.', tools: [], skills: [] }, rev: 1 }),
];

const imported = (now: number, name: string, ago: number) => ({ kind: 'claude-code', name, hash: 'c0ffee', at: at(now, ago) });
const skillMd = (name: string, description: string, body: string) => `---\nname: ${name}\ndescription: ${description}\n---\n\n${body}\n`;

// A reference file long enough to scroll in its pane, with a line too long to fit it.
const longReference = (title: string, lines: number) =>
  [`# ${title}`, '', ...Array.from({ length: lines }, (_, i) =>
    i % 40 === 39
      ? `- see https://example.com/docs/${title.toLowerCase().replace(/\W+/g, '-')}/section-${i}/with/a/path/that/has/no/spaces/at/all/and/keeps/going/until/it/must/wrap/somewhere?token=${'x'.repeat(48)}`
      : `${i + 1}. ${['Prefer the composition API', 'Keep a component to one job', 'Name a prop for what it means', 'Emit an event instead of reaching up', 'Style with the token, never a literal'][i % 5]} (rule ${i + 1}): ${'the reason is written once and not repeated elsewhere. '.repeat(1 + (i % 3))}`)].join('\n');

// The skills as the server holds them, each file with its text; the server lists skills by name.
type Held = Omit<ListedSkill, 'files'> & { files: Record<string, string> };
const heldSkills = (now: number): Held[] => [
  { name: 'frontend-design', description: 'Build distinctive, production-grade frontends that avoid generic AI aesthetics.', group: 'design',
    files: {
      'SKILL.md': skillMd('frontend-design', 'Build distinctive, production-grade frontends that avoid generic AI aesthetics.', '# Frontend design\n\nChoose a direction before you write a line: the tone, the one thing a person will remember, and what the screen is for.\n\n## Type\n\nPair a display face with a body face, and set both on a scale. See `references/typography.md`.\n\n## Colour\n\nOne dominant colour with sharp accents beats an evenly spread palette. See `references/color.md`.\n\n## Motion\n\nOne well-orchestrated load beats scattered micro-interactions.'),
      'references/typography.md': '# Typography\n\n- A display face for headings, a body face for text.\n- A scale of at most six sizes.\n- Line length between 60 and 75 characters.\n',
      'references/color.md': '# Colour\n\n- Name colours as tokens, never as literals.\n- Check contrast on both the surface and the text.\n',
    }, source: imported(now, 'frontend-design', 3 * 24 * 60), rev: 1 },
  { name: 'pinia', description: 'Pinia stores for Vue 3: setup stores, state, getters, actions and testing.', group: 'frontend',
    files: { 'SKILL.md': skillMd('pinia', 'Pinia stores for Vue 3: setup stores, state, getters, actions and testing.', '# Pinia\n\nDefine a store with `defineStore` and a setup function: refs are state, computed are getters, functions are actions.\n\n```ts\nexport const useCounter = defineStore(\'counter\', () => {\n  const n = ref(0);\n  const double = computed(() => n.value * 2);\n  const bump = () => n.value++;\n  return { n, double, bump };\n});\n```\n\nTest a store with `createPinia()` and `setActivePinia` before each test.') },
    source: imported(now, 'pinia', 3 * 24 * 60), rev: 1 },
  { name: 'release-notes', description: 'Write the notes of a release for the people who use it.', group: 'general',
    files: {
      'SKILL.md': skillMd('release-notes', 'Write the notes of a release for the people who use it.', '# Release notes\n\nStart from what changed for the person using it, not from what changed in the code. Group by what they can do now, what is fixed, and what to do when upgrading. Fill `template.md`.'),
      'template.md': '## What you can do now\n\n- \n\n## Fixed\n\n- \n\n## Upgrading\n\n- \n',
    }, rev: 2 },
  { name: 'security-review', description: 'Complete a security review of the pending changes on the current branch.', group: 'review',
    files: { 'SKILL.md': skillMd('security-review', 'Complete a security review of the pending changes on the current branch.', '# Security review\n\nRead the diff of the branch against its base, then the code around each change.\n\nLook for: injection, broken authentication, secrets in the change, unsafe deserialisation, missing authorisation checks, and logging of what should not be logged.\n\nReport each finding with the file and line, what an attacker could do, and how sure you are.') },
    source: imported(now, 'security-review', 9 * 24 * 60), rev: 1 },
  { name: 'shadcn-vue', description: 'Manages shadcn-vue components: adding, searching, fixing, styling and composing them.', group: 'design',
    files: {
      'SKILL.md': skillMd('shadcn-vue', 'Manages shadcn-vue components: adding, searching, fixing, styling and composing them.', '# shadcn-vue\n\nComponents are copied into the project, so they are yours to change. Add one with the CLI (`cli.md`), style it by rule (`rules/styling.md`), and compose it from smaller ones (`rules/composition.md`).\n\nThe full list of component rules is in `references/components.md`.'),
      'cli.md': '# CLI\n\n```sh\nnpx shadcn-vue@latest add button\nnpx shadcn-vue@latest search input\n```\n',
      'customization.md': '# Customization\n\nA new look is a variant in the component\'s `cva`, not a class pasted at the call site.\n',
      'rules/styling.md': '# Styling\n\nUse the semantic tokens. A colour, font or shadow is a token, never a literal.\n',
      'rules/forms.md': '# Forms\n\nGroup a field with its label and its error; never place an error away from its field.\n',
      'rules/composition.md': '# Composition\n\nA component takes layout classes; its look is its own.\n',
      'rules/icons.md': '# Icons\n\nOne icon set, one size per context.\n',
      'references/components.md': longReference('Component rules', 420),
    }, source: imported(now, 'shadcn-vue', 3 * 24 * 60), rev: 1 },
  { name: 'test-driven-development', description: 'Use when implementing any feature or bugfix, before writing implementation code.', group: 'engineering',
    files: {
      'SKILL.md': skillMd('test-driven-development', 'Use when implementing any feature or bugfix, before writing implementation code.', '# Test-driven development\n\n1. Write one failing test that says what should be true.\n2. Run it and watch it fail for the right reason.\n3. Write the least code that makes it pass.\n4. Refactor with the tests green.\n\nSee `testing-anti-patterns.md` for what to avoid.'),
      'testing-anti-patterns.md': '# Anti-patterns\n\n- Testing the mock instead of the behaviour.\n- A test that passes before the change.\n- Asserting on implementation details.\n',
    }, source: imported(now, 'test-driven-development', 9 * 24 * 60), rev: 1 },
  { name: 'vault.help', description: 'Answer where a vault artifact lives and how to create, check or fix it.', group: 'knowledge',
    files: {
      'SKILL.md': skillMd('vault.help', 'Answer where a vault artifact lives and how to create, check or fix it.', '# vault.help\n\nGive a direct, compact answer, then point to one to three canonical files. The artifact kinds and where each lives are in `references/artifacts.md`.'),
      'references/artifacts.md': '# Artifacts\n\n| Kind | Lives in |\n|---|---|\n| PRD | product-knowledge/prd/ |\n| Decision | product-knowledge/decisions/ |\n| Task | product-knowledge/tasks/ |\n',
    }, source: imported(now, 'vault.help', 2 * 24 * 60), rev: 1 },
  { name: 'vite', description: 'Vite build tool configuration, plugins, SSR and library mode, with the Rolldown-powered Vite 8.', group: 'frontend',
    files: { 'SKILL.md': skillMd('vite', 'Vite build tool configuration, plugins, SSR and library mode, with the Rolldown-powered Vite 8.', '# Vite\n\nConfigure in `vite.config.ts` with `defineConfig`. Plugins are listed in order; a plugin that transforms Vue goes before one that post-processes CSS.') },
    source: imported(now, 'vite', 3 * 24 * 60), rev: 1 },
  { name: 'vue', description: 'Vue 3 Composition API, script setup and reactivity.', group: 'frontend',
    files: {
      'SKILL.md': skillMd('vue', 'Vue 3 Composition API, script setup and reactivity.', '# Vue 3\n\nUse `<script setup lang="ts">`. State is a `ref`; a derived value is a `computed`; a side effect that follows state is a `watch`. Reactivity is explained in `references/reactivity.md`.'),
      'references/reactivity.md': '# Reactivity\n\nA `ref` holds one value; a `reactive` object tracks its properties. Destructuring a reactive object loses the tracking: use `toRefs`.\n',
    }, source: imported(now, 'vue', 3 * 24 * 60), rev: 1 },
];

// The skills as the snapshot lists them, by their files' paths and sizes; skillFile reads a file's text as it opens.
export const skillsSeed = (now: number): ListedSkill[] =>
  heldSkills(now).map((s) => ({ ...s, files: Object.entries(s.files).map(([path, text]) => ({ path, size: new TextEncoder().encode(text).length })) }));
export const skillFileSeed = (now: number) => async (name: string, path: string) => ({ text: heldSkills(now).find((s) => s.name === name)?.files[path] ?? '' });

const tool = (name: string, what: string) => ({ name, what });
const plain = (name: string, what: string, value?: string, extra: Partial<SourceVariable> = {}): SourceVariable => ({ name, what, secret: false, optional: false, address: false, state: value ? 'set' : 'unset', value: value ?? '', ...extra });
const secret = (name: string, what: string, state: SourceVariable['state'], extra: Partial<SourceVariable> = {}): SourceVariable => ({ name, what, secret: true, optional: false, address: false, state, value: '', ...extra });
const connection = (name: string, description: string, variables: SourceVariable[], extra: Partial<ConnectionView> = {}): ConnectionView => {
  const missing = variables.filter((v) => !v.optional && v.state === 'unset').map((v) => v.name);
  return { name, description, configured: missing.length === 0 && !extra.problem, missing, problem: '', variables, ...extra };
};

// The sources in the order the server lists them (its registry). The tools say what they answer; the connections are the
// awkward part: a default the environment serves, a staging and a production one, a stored secret that does not open.
export const toolsSeed = (): ToolSource[] => [
  { id: 'mongo', label: 'MongoDB', capability: 'data',
    tools: [tool('mongo_collections', 'List the collections of a database.'), tool('mongo_find', 'Find documents, read only; a filter and a limit.'), tool('mongo_count', 'Count the documents a filter matches.'), tool('mongo_aggregate', 'Run an aggregation pipeline that writes nothing.'), tool('mongo_schema', 'Sample a collection and say what fields its documents have.')],
    connections: [
      connection('default', '', [secret('MONGODB_URI', 'the connection string of the MongoDB deployment, login included; the address inside it is where it connects', 'environment')]),
      connection('staging', 'the staging cluster: orders and shops of the test stores, refreshed every night', [secret('MONGODB_URI', 'the connection string of the MongoDB deployment, login included; the address inside it is where it connects', 'set')]),
      connection('prod', 'the production cluster: every real shop; read the replicas only', [secret('MONGODB_URI', 'the connection string of the MongoDB deployment, login included; the address inside it is where it connects', 'unset')], { problem: 'MONGODB_URI cannot be opened: the key file is missing; copy its backup to the key file (mode 0600) and try again; nothing was cleared' }),
    ] },
  { id: 'starrocks', label: 'StarRocks', capability: 'data',
    tools: [tool('starrocks_query', 'Run a read-only SQL query on the warehouse.'), tool('starrocks_schema', 'List the tables of a database and the columns of one table.')],
    connections: [connection('default', '', [plain('STARROCKS_HOST', 'the host of the warehouse', undefined, { address: true }), plain('STARROCKS_PORT', 'its query port', undefined, { optional: true }), plain('STARROCKS_USER', 'a user that may only read'), secret('STARROCKS_PASSWORD', "that user's password", 'unset', { optional: true })])] },
  { id: 'bytebase', label: 'Bytebase', capability: 'data',
    tools: [tool('bytebase_databases', 'List the databases Bytebase manages.'), tool('bytebase_schema', 'The schema of one database.'), tool('bytebase_query', 'Run a read-only query through Bytebase.')],
    connections: [connection('default', '', [plain('BYTEBASE_URL', 'the address of Bytebase', 'https://bytebase.example.com', { address: true }), secret('BYTEBASE_SERVICE_KEY', 'a service account key that may only query', 'set')])] },
  { id: 'intercom', label: 'Intercom', capability: 'customers',
    tools: [tool('intercom_conversations', 'Search the conversations customers had with support.'), tool('intercom_conversation', 'One conversation in full.'), tool('intercom_contacts', 'Find a customer.')],
    connections: [connection('default', '', [secret('INTERCOM_TOKEN', 'an access token that may only read', 'environment')])] },
  { id: 'amplitude', label: 'Amplitude', capability: 'customers',
    tools: [tool('amplitude_events', 'The events the product sends.'), tool('amplitude_segmentation', 'Count an event over time, by a property.'), tool('amplitude_funnel', 'How many users get through a series of events.'), tool('amplitude_retention', 'How many users come back.'), tool('amplitude_user_search', 'Find a user.'), tool('amplitude_user_activity', 'What one user did.')],
    connections: [connection('default', '', [plain('AMPLITUDE_API_KEY', 'the project key'), secret('AMPLITUDE_SECRET_KEY', 'the project secret', 'unset')])] },
  { id: 'grafana', label: 'Grafana', capability: 'observe',
    tools: [tool('grafana_datasources', 'List the datasources Grafana reads.'), tool('grafana_promql', 'Run a PromQL query, instant or over a range.'), tool('grafana_alerts', 'The alert rules and what is firing.'), tool('grafana_dashboards', 'Search dashboards and read what a panel queries.')],
    connections: [
      connection('default', '', [plain('GRAFANA_URL', 'the address of the Grafana server', 'https://grafana.internal.example.com', { address: true }), secret('GRAFANA_TOKEN', 'a service-account token that may only read; with it no user and password are needed', 'environment', { optional: true }), plain('GRAFANA_USER', 'a Grafana user, when there is no token', undefined, { optional: true }), secret('GRAFANA_PASSWORD', "that user's password, when there is no token", 'unset', { optional: true })]),
      connection('eu-west', 'the Grafana of the EU region, behind a gateway with a very long address that has to wrap and never push the page sideways', [plain('GRAFANA_URL', 'the address of the Grafana server', 'https://grafana.eu-west-1.internal.example.com/tenants/6f1d2c3a-9b4e-4d77-8a21-5c0e7b9f3a10/proxy?profile=read-only&trace=on', { address: true }), secret('GRAFANA_TOKEN', 'a service-account token that may only read; with it no user and password are needed', 'unset', { optional: true }), plain('GRAFANA_USER', 'a Grafana user, when there is no token', undefined, { optional: true }), secret('GRAFANA_PASSWORD', "that user's password, when there is no token", 'unset', { optional: true })]),
    ] },
  { id: 'coroot', label: 'Coroot', capability: 'observe',
    tools: [tool('coroot_applications', 'The applications and their standing.'), tool('coroot_incidents', 'The incidents that were open in a period.'), tool('coroot_application', 'One application: its SLOs, instances and dependencies.'), tool('coroot_logs', 'Logs of one application.')],
    connections: [connection('default', '', [plain('COROOT_PUBLIC_URL', 'the address of Coroot', undefined, { address: true }), secret('COROOT_API_KEY', 'a read-only API key', 'unset'), plain('COROOT_PROJECT', 'the project to read', undefined)])] },
  { id: 'cloudwatch', label: 'CloudWatch', capability: 'observe',
    tools: [tool('cloudwatch_log_groups', 'List the log groups.'), tool('cloudwatch_logs', 'Read the events of a log group.'), tool('cloudwatch_insights', 'Run a Logs Insights query.')],
    connections: [connection('default', '', [plain('AWS_REGION', 'the region to read', 'eu-west-1', { address: true }), secret('AWS_ACCESS_KEY_ID', 'the key id of a user that may only read logs', 'environment'), secret('AWS_SECRET_ACCESS_KEY', 'its secret', 'environment')])] },
];

// The requests the Assistant raised when the owner asked it to connect a source, each a variable with no value yet. The plain
// one comes first: the secret's item says where its value will be sent. The production one offers the removal of its connection.
export const credentialItems = (now: number): InboxItem[] => [
  inboxRecord({
    id: 'i7', home: 'assistant', cardId: 'assistant', kind: 'credential', title: 'Credential for grafana/eu-west: GRAFANA_TOKEN',
    detail: 'The **Grafana** connection eu-west needs a service-account token that may only read; with it no user and password are needed (`GRAFANA_TOKEN`). It is sent to https://grafana.eu-west-1.internal.example.com/tenants/6f1d2c3a-9b4e-4d77-8a21-5c0e7b9f3a10/proxy?profile=read-only&trace=on. Type it into the field of this item: it is stored encrypted, and no agent sees it.',
    options: [optionOf('remove-connection', 'Remove this connection')], status: 'open', source: 'grafana', connection: 'eu-west', variable: 'GRAFANA_TOKEN', secret: true, createdAt: at(now, 4), rev: 1,
  }),
  inboxRecord({
    id: 'i8', home: 'assistant', cardId: 'assistant', kind: 'credential', title: 'Credential for mongo/prod: MONGODB_URI',
    detail: 'The **MongoDB** connection prod needs the connection string of the MongoDB deployment, login included; the address inside it is where it connects (`MONGODB_URI`). It is sent to the address inside the value, or the one MongoDB fixes. Type it into the field of this item: it is stored encrypted, and no agent sees it.',
    options: [optionOf('remove-connection', 'Remove this connection')], status: 'open', source: 'mongo', connection: 'prod', variable: 'MONGODB_URI', secret: true, createdAt: at(now, 3), rev: 1,
  }),
  inboxRecord({
    id: 'i12', home: 'assistant', cardId: 'assistant', kind: 'credential', title: 'Credential for coroot/default: COROOT_PUBLIC_URL',
    detail: 'The **Coroot** connection default needs the address of Coroot (`COROOT_PUBLIC_URL`). Type it into the field of this item: it is stored as typed and shown on the Tools page.',
    options: [], status: 'open', source: 'coroot', connection: 'default', variable: 'COROOT_PUBLIC_URL', secret: false, createdAt: at(now, 2), rev: 1,
  }),
];

// What a card of an agent was given, as the server's Access computes it from the agent and the library (made up here).
export function givenOf(agentId: string): Partial<ContextGiven> {
  const agent = agentsSeed().find((a) => a.id === agentId);
  if (!agent) return {};
  return { skills: agent.loadout.skills.map((n) => `library:${n}`), claudeTools: agent.loadout.tools, missing: [] };
}

// The writes an agent asked to make that move a rail, waiting on the owner (the server words each item). This one is the
// awkward case: a program with a long path and arguments, a plain environment value that is itself a program's, and a field
// that has no value yet.
export const changeItems = (now: number): InboxItem[] => [
  inboxRecord({
    id: 'i10', home: 'author', cardId: 'author-chair', kind: 'change', title: 'The author chain asks to write agent researcher',
    detail: [
      'The author chain asks to write agent `researcher`. These fields move a rail, so it is not written until you answer:',
      '',
      '- **instructions**: `Search the index with the search tool and quote what it returns.` → `Search the index with the search tool, quote what it returns, and when it returns nothing, widen the query twice before you say so; never answer from memory what the index could have told you.`',
      '',
      'Apply writes it as shown. Not now writes nothing, and the agent is told.',
    ].join('\n'),
    options: [optionOf('apply', 'Apply'), optionOf('not_now', 'Not now')], status: 'open', createdAt: at(now, 2), rev: 1,
  }),
];

const AUTHOR: Actor = actorOf('agent', 'author-chair');
const OWNER: Actor = actorOf('owner', 'owner@example.com', 'web');
const SEED: Actor = actorOf('remit', 'seed');
const long = (word: string, n: number) => `"${word.repeat(n)}`.slice(0, 600) + '…';

// What was written to some of the records above, by `kind/name`, newest first, as the server keeps it. Awkward cases on
// purpose: a write the owner let through, one by Remit, a creation, a record with no history (an import from before it
// was kept), and one with many fields, values cut where the server cut them.
export const changesSeed = (now: number): Record<string, Change[]> => ({
  'agent/engineer': [
    changeRecord({ id: 'ch_e2', kind: 'agent', name: 'engineer', action: 'changed', by: actorOf('assistant', 'assistant'), at: at(now, 25),
      fields: [{ name: 'description', before: '"Writes the change with a test first."', after: '"Writes the change with a test first, in the repository’s own style, and opens nothing for review itself."' }] }),
    changeRecord({ id: 'ch_e1', kind: 'agent', name: 'engineer', action: 'created', by: AUTHOR, at: at(now, 2 * 24 * 60), fields: [] }),
  ],
  'playbook/bugfix': [
    changeRecord({ id: 'ch_p2', kind: 'playbook', name: 'bugfix', action: 'changed', by: SEED, at: at(now, 12 * 60), fields: [{ name: 'brief', before: '"Hire freely."', after: '"Hire freely. Ask the owner only what cannot be found."' }] }),
    changeRecord({ id: 'ch_p1', kind: 'playbook', name: 'bugfix', action: 'created', by: SEED, at: at(now, 9 * 24 * 60), fields: [] }),
  ],
  'skill/shadcn-vue': [
    changeRecord({ id: 'ch_s1', kind: 'skill', name: 'shadcn-vue', action: 'changed', by: AUTHOR, at: at(now, 6 * 60), fields: [
      { name: 'description', before: '"Manages shadcn-vue components."', after: '"Manages shadcn-vue components: adding, searching, fixing, styling and composing them."' },
      { name: 'files.SKILL.md', before: long('before ', 200), after: long('after ', 200) },
      { name: 'files.rules/forms.md', before: '"# Forms"', after: '"# Forms\\n\\nGroup a field with its label and its error."' },
      { name: 'files.rules/icons.md', before: '', after: '"# Icons"' },
      { name: 'files.rules/styling.md', before: '"# Styling"', after: '' },
      { name: 'files.rules/composition.md', before: '"# Composition"', after: '"# Composition\\n\\nA component takes layout classes."' },
    ] }),
  ],
});
