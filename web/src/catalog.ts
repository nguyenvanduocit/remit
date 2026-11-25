// The replica of what the server keeps for the whole app: every chain, the open inbox, where each chain stands (the board
// and each chain's standing), the playbooks, the agents and skills, the tools (sources and their connections), agent health, the workload, who is at work now, the signals board, the
// analyst's newest insights, the objectives with where each key result stands, the projects, the employees, the mentors and where each stands, what became of the newest events, the event signals (without their keys), the sensors.
// The core keeps it in step (replica.ts): each snapshot replaces it, then each push changes one record, in the order the
// server lists them. Drawn here as the core hands its models, for the page's life (every page counts from them); nothing
// here is decided in the browser.
import { computed, type ComputedRef } from 'vue';
import type { Agent, AtWork, Board, Boards, CapabilityView, Chain, ChainRead, ChainStanding, Decision, EmployeeRead, ObjectiveView, EventRecord, Feedback, Health, InboxItem, ClientKey, Extension, Mentor, MentorStanding, Playbook, Project, SignalBoard, ListedSkill, ToolSource, EventSignal, Sensor, Workload } from './core/generated/types.generated';
import type { Model } from './core/generated/replica/types.generated';
import { inOrder, oneOf } from './core/client';
import { accountLabel } from './accounts';
import { coreState, useModel } from './replica';

export interface Catalog {
  readonly loaded: boolean; // the first snapshot is in
  readonly chains: readonly ChainRead[]; // newest first
  readonly inbox: readonly InboxItem[];
  readonly board: Board | null;
  readonly standings: Readonly<Record<string, ChainStanding>>; // by chain id
  readonly playbooks: readonly Playbook[];
  readonly agents: readonly Agent[]; // by id
  readonly skills: readonly ListedSkill[]; // by name
  readonly sources: readonly ToolSource[]; // in the order the server lists them
  readonly capabilities: readonly CapabilityView[];
  readonly health: Health | null;
  readonly workload: Workload | null;
  readonly atWork: AtWork | null; // every seat at work or waiting across the chains and the Assistant, in the server's order
  readonly signals: SignalBoard | null; // what Remit waits for and does on its own, as the server lays it out
  readonly boards: Boards | null; // the columns of the Subscriptions, Feedback and Crew boards, as the server lays them out
  readonly insights: readonly Decision[]; // newest first
  readonly events: readonly EventRecord[]; // the newest events with their firings, newest first, the newest 200 (EVENTS_KEPT), without what arrived
  readonly eventSignals: readonly EventSignal[]; // every one, newest made first, never with its key
  readonly sensors: readonly Sensor[]; // every one, newest made first: the standing polls, the scripts of chains, the clocks and the plugins' kinds
  readonly clientKeys: readonly ClientKey[]; // every one, newest made first, never with its value
  readonly extensions: readonly Extension[]; // every plugin source, bundled first, disabled ones too
  readonly feedback: readonly Feedback[]; // newest first, the newest 200 (FEEDBACK_KEPT)
  readonly objectives: readonly ObjectiveView[]; // every one, newest made first
  readonly projects: readonly Project[]; // every one, archived ones too, by name
  readonly employees: readonly EmployeeRead[]; // every one, oldest hired first, as the server lists them
  readonly mentors: readonly Mentor[]; // the pool, removed ones too, oldest added first, as the server lists them
  readonly mentorStandings: Readonly<Record<string, MentorStanding>>; // where each mentor stands, by mentor id
}

const listOf = <T>(model: Model): ComputedRef<T[]> => inOrder(useModel<T>(model));
// A model keyed by an id, as a record by that id.
const byKey = <T>(model: Model): ComputedRef<Readonly<Record<string, T>>> => {
  const m = useModel<T>(model);
  return computed(() => Object.fromEntries(m.rows));
};

const chains = listOf<ChainRead>({ model: 'chains' });
const inbox = listOf<InboxItem>({ model: 'inbox' });
const standings = byKey<ChainStanding>({ model: 'standings' });
const playbooks = listOf<Playbook>({ model: 'playbooks' });
const agents = listOf<Agent>({ model: 'agents' });
const skills = listOf<ListedSkill>({ model: 'skills' });
const sources = listOf<ToolSource>({ model: 'sources' });
const capabilities = listOf<CapabilityView>({ model: 'capabilities' });
const insights = listOf<Decision>({ model: 'insights' });
const events = listOf<EventRecord>({ model: 'events' });
const eventSignals = listOf<EventSignal>({ model: 'eventSignals' });
const sensors = listOf<Sensor>({ model: 'sensors' });
const clientKeys = listOf<ClientKey>({ model: 'clientKeys' });
const extensions = listOf<Extension>({ model: 'extensions' });
const feedback = listOf<Feedback>({ model: 'feedback' });
const objectives = listOf<ObjectiveView>({ model: 'objectives' });
const projects = listOf<Project>({ model: 'projects' });
const employees = listOf<EmployeeRead>({ model: 'employees' });
const mentors = listOf<Mentor>({ model: 'mentors' });
const mentorStandings = byKey<MentorStanding>({ model: 'mentorStandings' });
const board = useModel<Board>({ model: 'board' });
const health = useModel<Health>({ model: 'health' });
const workload = useModel<Workload>({ model: 'workload' });
const atWork = useModel<AtWork>({ model: 'atWork' });
const signals = useModel<SignalBoard>({ model: 'signals' });
const boards = useModel<Boards>({ model: 'boards' });

export const catalog: Catalog = {
  get loaded() {
    return coreState.value?.loaded ?? false;
  },
  get chains() {
    return chains.value;
  },
  get inbox() {
    return inbox.value;
  },
  get board() {
    return oneOf(board);
  },
  get standings() {
    return standings.value;
  },
  get playbooks() {
    return playbooks.value;
  },
  get agents() {
    return agents.value;
  },
  get skills() {
    return skills.value;
  },
  get sources() {
    return sources.value;
  },
  get capabilities() {
    return capabilities.value;
  },
  get health() {
    return oneOf(health);
  },
  get workload() {
    return oneOf(workload);
  },
  get atWork() {
    return oneOf(atWork);
  },
  get signals() {
    return oneOf(signals);
  },
  get boards() {
    return oneOf(boards);
  },
  get insights() {
    return insights.value;
  },
  get events() {
    return events.value;
  },
  get eventSignals() {
    return eventSignals.value;
  },
  get sensors() {
    return sensors.value;
  },
  get clientKeys() {
    return clientKeys.value;
  },
  get extensions() {
    return extensions.value;
  },
  get feedback() {
    return feedback.value;
  },
  get objectives() {
    return objectives.value;
  },
  get projects() {
    return projects.value;
  },
  get employees() {
    return employees.value;
  },
  get mentors() {
    return mentors.value;
  },
  get mentorStandings() {
    return mentorStandings.value;
  },
};

// How many events the server lists, and so how many the core keeps (Snapshot.events).
export const EVENTS_KEPT = 200;
// How much feedback the server lists, and so how much the core keeps (Snapshot.feedback).
export const FEEDBACK_KEPT = 200;

// The agents cannot run (no account can run them): the bar under the top bar shows, and views make room for it.
export const healthBad = computed(() => !!catalog.health && !catalog.health.ok && !!catalog.health.checkedAt);
// Whether agents run on more than one account, which is when a card's account is worth saying.
// What the owner calls the account an agent runs on: its address, else its id.
export const accountNamed = (id: string): string => {
  const a = catalog.health?.accounts.find((x) => x.id === id);
  return a ? accountLabel(a) : id;
};
export const manyAccounts = computed(() => (catalog.health?.accounts.length ?? 0) > 1);
// The chains as stored, newest first, for what reads a chain and not when its Chair comes back.
export const chainRecords = computed((): Chain[] => catalog.chains.map((c) => c.chain));
// The chain the server lists by id, as stored.
export const listedChain = (id: string): Chain | undefined => catalog.chains.find((c) => c.chain.id === id)?.chain;
// The server lists the chain: it exists. One it does not list was deleted, or never was.
export const listed = (chainId: string): boolean => catalog.chains.some((c) => c.chain.id === chainId);
// Where a chain stands, as the server says; one it has not stood yet (just created, before its push) stands nowhere.
const NOWHERE: ChainStanding = { chainId: '', text: '', tone: 'queued', working: false, waits: 0, unseen: false, asks: [], from: '', rev: 0 };
export const standingOf = (chainId: string): ChainStanding => standings.value[chainId] ?? NOWHERE;
// The line a card under a chain's title gives of it: why it waits (the server's), else the start of its summary, else of its goal.
export const gistOf = (c: Chain, st: ChainStanding): string => st.why || (c.summary ? c.summary.replace(/[#*`]/g, '').slice(0, 140) : c.goal.slice(0, 140));

// The project an id names, archived ones too; none for an id the server does not list (ADR-0108).
export const projectOf = (id: string | undefined): Project | undefined => (id ? catalog.projects.find((p) => p.id === id) : undefined);

// What a capability lets a chain do, in the owner's words, as the server's catalog says; a name it no longer has shows as itself.
export const capabilityLabel = (id: string): string => catalog.capabilities.find((c) => c.id === id)?.label ?? id;

// Where a mentor stands, as the server last said; undefined before the server stood them.
export const standingOfMentor = (mentorId: string): MentorStanding | undefined => mentorStandings.value[mentorId];
export const feedbackUnread = computed(() => catalog.feedback.filter((f) => !f.readAt).length);
// What a client key is called, for an act that names it by its id; a key since deleted reads as deleted.
export const clientKeyNameOf = (id: string | undefined): string => catalog.clientKeys.find((k) => k.id === id)?.name ?? `deleted key ${id ?? ''}`.trim();
