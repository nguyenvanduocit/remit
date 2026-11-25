// The server's side for the verify pages and the tests: the page's real client core (replica.ts) over the core's
// FakeServer (crates/client-web, `fake`), which answers each call from the page's seed and pushes what the page plays.
// A seed answers as the calls are asked (`Api`: the same names and arguments, a value or a promise of one); a call it
// does not answer fails, saying so. A verify page runs the core's `fake` build in the SharedWorker its tabs share, as the
// app does (verify/fake.ts, verify/worker.ts); a test runs the same build, read from disk, in the page itself
// (verify/fakeNode.ts); the rest is here.
import { createApi, type Api, type PushArgs } from '../src/core/generated/api.generated';
import { createCore, createPage, type Binding, type Keeping, type Open } from '../src/core/client';
import type { Boards, GroupColumn, Snapshot } from '../src/core/generated/types.generated';
import type { RefusalCode, Told } from '../src/core/generated/replica/types.generated';
import { coreState, startCore } from '../src/replica';
import { watch } from 'vue';
import { cardRecord } from './records';

/** The core's module, as wasm-bindgen binds it with `fake`. */
export interface FakePkg {
  Core: { fake(start: string, server: FakeServerHandle): { tab(sink: (told: Told[]) => void): Binding; free(): void } };
  FakeServer: new (answer: Answer) => FakeServerHandle;
}

/** A call of the contract as the fake server hands it to the seed (`{ method, args }`). */
export interface FakeCall {
  method: string;
  args: { [k: string]: unknown };
}

/** How the seed answers a call: `{ ok }` or `{ err }`, now or as a promise. */
export type Answer = (call: FakeCall) => unknown;

/** What a page plays the server doing. */
export interface FakeServerHandle {
  push(home: string | undefined, push: string): void;
  lose(): void;
  refuse(code: RefusalCode, reason: string): void;
  goDown(): void;
  comeUp(): void;
}

/** What a page answers: any of the calls, as `Api` asks them. */
export type Answers = { [M in keyof Api]?: (...args: Parameters<Api[M]>) => ReturnType<Api[M]> | Awaited<ReturnType<Api[M]>> };

/** The pushes as a page plays them: by the name the old handlers had, with the one record each carries. */
type Camel<S extends string> = S extends `${infer A}_${infer B}` ? `${A}${Capitalize<Camel<B>>}` : S;
type Pushes = { [K in keyof PushArgs as Camel<K & string>]: PushArgs[K][keyof PushArgs[K]] };

// The one field each push carries its record in (PushArgs), so a page names only the record.
const PUSH_FIELD: { [K in keyof PushArgs]: keyof PushArgs[K] } = {
  chain_updated: 'chain', chain_deleted: 'gone', card_updated: 'card', card_shown: 'shown', edge_added: 'edge',
  decision_added: 'decision', landing_updated: 'landing', repo_updated: 'view', workflow_updated: 'workflow', insight_added: 'insight', assistant_feed_updated: 'feed',
  at_work_updated: 'atWork',
  draft_updated: 'draft', draft_deleted: 'gone', event_added: 'event', event_updated: 'event',
  event_deleted: 'gone', event_delta: 'delta', health_updated: 'health', workload_updated: 'workload',
  signal_board_updated: 'board', boards_updated: 'boards', event_read: 'event', event_signal_updated: 'eventSignal', event_signal_deleted: 'gone',
  sensor_updated: 'sensor', sensor_deleted: 'gone', client_key_updated: 'key', client_key_deleted: 'gone',
  extension_updated: 'extension', extension_deleted: 'gone', feedback_updated: 'feedback', objective_updated: 'view', project_updated: 'project',
  employee_updated: 'employee', mentor_updated: 'mentor', help_updated: 'help', inbox_updated: 'item',
  playbook_updated: 'playbook', playbook_deleted: 'gone', agent_updated: 'agent', agent_deleted: 'gone',
  skill_updated: 'skill', skill_deleted: 'gone', source_updated: 'source', board_updated: 'board',
  chain_standing_updated: 'standing', agent_standing_updated: 'agent', mentor_standing_updated: 'mentor',
  verification_updated: 'verification', retrospective_added: 'retrospective', home_reset: 'reset', home_shown: 'mark',
};

const snake = (name: string): string => name.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`);
const camel = (name: string): string => name.replace(/_([a-z])/g, (_, c: string) => c.toUpperCase());

// Each call's argument names in the order `Api` takes them, read from the generated client itself: asked with its
// positions as values, it says which field each one fills.
const order = new Map<string, string[]>();
{
  let seen: { [k: string]: unknown } = {};
  const probe = createApi(((_: string, args: { [k: string]: unknown }) => ((seen = args), Promise.resolve())) as never) as { [k: string]: (...a: unknown[]) => unknown };
  for (const [name, fn] of Object.entries(probe)) {
    fn(...Array.from({ length: 8 }, (_, i) => `#${i}`));
    order.set(snake(name), Object.entries(seen).sort(([, a], [, b]) => String(a).localeCompare(String(b))).map(([field]) => field));
  }
}

const failure = (e: unknown) => {
  const { code, detail } = (e as { code?: string; detail?: unknown } | null) ?? {};
  const message = e instanceof Error ? e.message : ((e as { message?: string } | null)?.message ?? String(e));
  return { err: { code: code ?? 'FAILED', message, ...(detail === undefined ? {} : { detail }) } };
};

export interface Fake {
  /** The server pushing a change, as the link would deliver it. */
  push<K extends keyof Pushes>(name: K, record: Pushes[K]): void;
  /** The session goes under, as a lost connection does; the core dials again. */
  lose(): void;
  /** The server turns the page away from now on (a revoked key, an older build); the session up goes under. */
  refuse(code: RefusalCode, reason: string): void;
  /** The server cannot be reached until `comeUp`: the session up goes under and every dial fails. */
  goDown(): void;
  comeUp(): void;
  /** The calls asked, by `Api` name, in order. */
  asked: string[];
  /** The request id each call carried, in the same order: a changing call's, none for a reading one. */
  requests: (string | undefined)[];
}

let stopWatching: (() => void) | undefined;

// What the real server always answers, for a seed that leaves it out: no seats at work, the homes a page shows taken (and
// printed, so a check can read which the core names), and
// the core's minute of link telemetry kept (printed, so a check can read what the core counted).
const DEFAULTS: Partial<Answers> = {
  atWork: () => ({ seats: [], rev: 1 }),
  homesShown: (homes) => {
    console.info(`homesShown ${JSON.stringify(homes)}`);
    return {};
  },
  linkReport: (report) => {
    console.info(`linkReport ${JSON.stringify(report)}`);
    return {};
  },
};

/** Where the core runs and its fake server, for a seed that answers `answer`. */
export type Host = (answer: Answer) => { server: FakeServerHandle; open: Open };

/** A core in the page itself, over a fake server of its own: the page is its one tab. */
export const inPage =
  (pkg: FakePkg): Host =>
  (answer) => {
    const server = new pkg.FakeServer(answer);
    return { server, open: onePage(pkg, server) };
  };

/** A core of the page's own over `server`, whose one tab the page is. */
const onePage =
  (pkg: FakePkg, server: FakeServerHandle): Open =>
  (sink) => {
    const core = pkg.Core.fake(START, server);
    const tab = core.tab(sink);
    return {
      ask: (a) => tab.ask(a),
      free: () => {
        tab.free();
        core.free();
      },
    };
  };

/** Starts the page's core on a fake server answering from `answers`, where `host` runs it. */
export function startFake(pkg: FakePkg, answers: Answers, host: Host, keeping?: Keeping): Fake {
  const asked: string[] = [];
  const requests: (string | undefined)[] = [];
  const answer = ({ method, args }: FakeCall) => {
    const name = camel(method) as keyof Api;
    asked.push(name);
    requests.push((args.request as { id?: string } | undefined)?.id);
    const fn = (answers[name] ?? DEFAULTS[name]) as ((...a: unknown[]) => unknown) | undefined;
    if (!fn) return failure(new Error(`${name} is not answered on this page`));
    const positional = (order.get(method) ?? []).map((field) => args[field]);
    // A value is answered at once; a promise (a seed playing a slow server) when it settles.
    try {
      const value = fn(...positional);
      if (value instanceof Promise) return value.then((v) => ({ ok: v ?? {} }), failure);
      return { ok: value ?? {} };
    } catch (e) {
      return failure(e);
    }
  };
  const { server, open } = host(answer);
  // A seed the core cannot read (a record missing a field the contract requires) leaves the link stale: say why, and
  // which of the snapshot's records it is. One watch per page, set before the core starts (a seed answered at once goes
  // stale inside startCore): a newer fake replaces the older's.
  stopWatching?.();
  stopWatching = watch(
    () => coreState.value?.phase === 'stale' && coreState.value.why,
    async (why) => {
      if (!why) return;
      console.error(`fakeServer: the core could not load the snapshot: ${why}`);
      // A page playing a snapshot that fails (the stale link it shows) has none to look into.
      const snap = await Promise.resolve()
        .then(() => answers.appSnapshot?.())
        .catch(() => undefined);
      if (snap) for (const bad of unreadable(pkg, snap)) console.error(`fakeServer: the core cannot read snapshot.${bad}`);
    },
  );
  startCore(open, keeping);
  return {
    push: (name, record) => {
      const wire = snake(name) as keyof PushArgs;
      server.push(undefined, JSON.stringify({ push: wire, args: { [PUSH_FIELD[wire]]: record } }));
    },
    lose: () => server.lose(),
    refuse: (code, reason) => server.refuse(code, reason),
    goDown: () => server.goDown(),
    comeUp: () => server.comeUp(),
    asked,
    requests,
  };
}

/** How a fake page starts its core: no server to dial, no build to name. */
export const START = JSON.stringify({ nodeId: 'page', publicKey: '', salt: 'fake' });

// Whether a core of its own loads this snapshot.
function loads(pkg: FakePkg, snap: Snapshot): boolean {
  const page = createPage();
  const server = new pkg.FakeServer(({ method }) => (method === 'app_snapshot' ? { ok: snap } : { err: { code: 'FAILED', message: 'probe' } }));
  const core = createCore(onePage(pkg, server), () => {}, page);
  core.ask({ ask: 'connect' });
  const ok = page.state.value?.phase === 'live';
  core.close();
  return ok;
}

// The parts of a snapshot the core cannot read: each field alone over an empty one, and of a list, each record alone.
function unreadable(pkg: FakePkg, snap: Snapshot): string[] {
  const bad: string[] = [];
  for (const [key, value] of Object.entries(snap)) {
    if (loads(pkg, snapshotOf({ [key]: value }))) continue;
    if (!Array.isArray(value)) bad.push(key);
    else value.forEach((record, i) => loads(pkg, snapshotOf({ [key]: [record] })) || bad.push(`${key}[${i}] ${JSON.stringify(record).slice(0, 160)}`));
  }
  return bad;
}

// The columns of the boards as the server lays them out (crates/orch/src/boards.rs): the Crew boards' from the groups the seed's
// records name, kinds of work by name with general last, capabilities in the order the sources come.
function boardsOf(seed: Partial<Snapshot>): Boards {
  const work = (groups: string[], what: string): GroupColumn[] =>
    [...new Set(groups)].sort((a, b) => Number(a === 'general') - Number(b === 'general') || (a < b ? -1 : a > b ? 1 : 0)).map((key) => ({ key, label: key, icon: 'list', empty: `No ${what} for ${key} work matches.` }));
  const capabilities = [...new Set((seed.sources ?? []).map((s) => s.capability))];
  return {
    subscriptions: [
      { key: 'attention', label: 'Held back or due now', icon: 'alert', empty: 'A subscription stands here once its time has come, or when a firing of it failed or something holds it back.' },
      { key: 'coming', label: 'Fires at its time or at a reading', icon: 'clock', empty: 'A subscription stands here while it waits for its time or its signal, and while the chain it started runs.' },
      { key: 'idle', label: 'Paused or over: fires no more', icon: 'pause', empty: 'A subscription stands here while it is paused, and once it has no next time.' },
    ],
    subscriptionKinds: [
      { kind: 'check_back', plural: 'Check-backs', noun: 'Check-back', says: 'Checks back on a chain', icon: 'clock', startsHidden: false },
      { kind: 'watch', plural: 'Watches', noun: 'Watch', says: 'Watches a signal', icon: 'eye', startsHidden: false },
      { kind: 'routine', plural: 'Routines', noun: 'Routine', says: 'Starts a chain', icon: 'repeat', startsHidden: false },
      { kind: 'event', plural: 'On events', noun: 'On events', says: 'Acts on an event', icon: 'inbox', startsHidden: false },
      { kind: 'automation', plural: 'Automations', noun: 'Automation', says: 'Does something on its own', icon: 'spark', startsHidden: true },
    ],
    feedback: [
      { key: 'practice', label: 'How I work', icon: 'gear', empty: 'What an agent says of how it is asked to work lands here.' },
      { key: 'difficulty', label: 'Difficulty', icon: 'alert', empty: "What got in an agent's way lands here." },
      { key: 'complaint', label: 'Complaint', icon: 'comment', empty: 'What an agent complains of lands here.' },
      { key: 'suggestion', label: 'Suggestion', icon: 'spark', empty: 'What an agent would change lands here.' },
    ],
    playbooks: work((seed.playbooks ?? []).map((p) => p.group), 'playbook'),
    agents: work((seed.agents ?? []).map((a) => a.group), 'agent'),
    skills: work((seed.skills ?? []).map((s) => s.group), 'skill'),
    sources: capabilities.map((key) => ({ key, label: key, icon: 'terminal', empty: `No source that gives ${key} matches.` })),
    rev: 1,
  };
}

/** A whole snapshot from the parts a page seeds: what it leaves out is empty, as a server with nothing in it says. */
export function snapshotOf(seed: Partial<Snapshot>): Snapshot {
  const at = new Date().toISOString();
  const card = cardRecord({ id: 'assistant-card', home: 'assistant', name: 'Assistant', origin: 'assistant', status: 'idle', createdAt: at, updatedAt: at });
  const standing = { cardId: card.id, home: card.home, label: '', tone: 'idle', doing: '', doingTone: 'idle', work: '', warms: false, waiting: [], children: [], with: [], peers: [], branch: { agents: 0, turns: 0, working: false }, asks: [], stopSays: '', remembers: true, rev: 1 } as const;
  return {
    who: { kind: 'owner' },
    chains: [], inbox: [], standings: [], playbooks: [], agents: [], skills: [], sources: [], capabilities: [], insights: [], events: [], eventSignals: [],
    sensors: [], clientKeys: [], extensions: [], feedback: [], objectives: [], projects: [], employees: [], mentors: [], mentorStandings: [],
    board: { columns: [], archived: 0, needsYou: 0, waiting: [], rev: 1 },
    health: { ok: true, error: '', accounts: [], unreadable: 0, rev: 1 },
    workload: { running: 0, capacity: 3, pressure: '', queued: 0, waiting: 0, rev: 1 },
    signals: { sources: [], subscriptions: [], signals: [], signalsMore: 0, held: 0, rev: 1 },
    boards: boardsOf(seed),
    assistant: { assistant: { id: card.home, cardId: card.id, createdAt: at }, card: { ...card }, agent: { ...standing, waiting: [], children: [], with: [], peers: [], asks: [], branch: { ...standing.branch } }, drafts: [] },
    assistantFeed: { lines: [], unseen: 0 },
    ...seed,
  };
}
