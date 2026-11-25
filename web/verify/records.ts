// Whole records for the verify seeds: a seed names what its case is about and takes the rest as an empty record, the
// way the server's types default it (crates/model: `#[serde(default)]`), so the defaults of each record live here once.
import type { FiringLine, ReadingLine, SignalBoard, SignalLine, SubscriptionLine, AccountHealth, Activity, Actor, ActorKind, Agent, Card, Ceiling, Chain, ChainRead, Change, CheckBack, ClientKey, Decision, DecisionRead, Draft, Employee, EmployeeRead, Event, EventKind, EventRead, EventSignal, EventStatus, Feedback, Finding, Help, Horizon, InboundEvent, InboxItem, InboxOption, Landing, LandingRead, LandingVersion, Loadout, Mentor, NearMiss, Objective, PermissionAsk, Playbook, Project, RepoWork, Retrospective, RetrospectiveRead, Retry, Seed, Sensor, Workflow, WorkflowStep } from '../src/core/generated/types.generated';

/** Who did something: the owner by his account and channel, an agent or the Assistant by card id, Remit by its part. */
export const actorOf = (kind: ActorKind, who = '', via = ''): Actor => ({ kind, who, via });

/** A chain: its id, title, status and times; what else it carries, as the case needs. */
export const chainRecord = (fields: Pick<Chain, 'id' | 'title' | 'status' | 'createdAt' | 'updatedAt'> & Partial<Chain>): Chain => ({
  goal: '', kind: 'work', capabilities: [], origin: 'owner', proposedBy: '', routineId: '', eventId: '', replay: '',
  chairId: '', workdir: '', summary: '', repos: [], rev: 1,
  ...fields,
});

/** A chain as a channel reads it: the chain, and when its Chair comes back, when it does. */
export const chainRead = (chain: Chain, checkBack?: CheckBack): ChainRead => (checkBack ? { chain, checkBack } : { chain });

/** A card: its id, home, name, origin, status and times; what else it carries, as the case needs. */
export const cardRecord = (fields: Pick<Card, 'id' | 'home' | 'name' | 'origin' | 'status' | 'createdAt' | 'updatedAt'> & Partial<Card>): Card => ({
  agent: '', employee: '', signs: '', why: '', brief: '', deliverable: '', criteria: [], awaitingReview: false, model: '', effort: '', account: '',
  mode: '', cwd: '', sessionId: '', activity: '', plan: [], ctxUsed: 0, ctxSize: 0, turns: 0, queued: 0, output: '', error: '', rev: 1,
  ...fields,
});

/** A decision: its id, chain, kind, text and time; who and why, as the case needs. */
export const decisionRecord = (fields: Pick<Decision, 'id' | 'chainId' | 'kind' | 'text' | 'createdAt'> & Partial<Decision>): Decision => ({
  why: '', cardId: '', item: '', overturns: '', option: '', stance: '',
  ...fields,
});

/** A decision as a chain's stream reads it: whether it is an outcome of the chain's work is the server's to say. */
export const decisionRead = (decision: Decision, outcome: boolean): DecisionRead => ({ decision, outcome });

/** A repository a chain works in, by its name; its branch, as the case needs. */
export const repoRecord = (fields: Pick<RepoWork, 'repo'> & Partial<RepoWork>): RepoWork => ({
  project: '', host: '', path: '', branch: '', base: '',
  ...fields,
});

/** A landing: its id, chain, repository, state and time, and its version's number and title; the rest the queue writes, as
 * the case needs. */
// A landing as a chain's screen reads it, with the release that carried it and, for a revert still on its way, the landing it
// takes back: what the server reads beside each.
export const landingRead = (landing: Landing, more: Omit<LandingRead, 'landing'> = {}): LandingRead => ({ landing, ...more });

export const landingRecord = ({ version, ...fields }: Pick<Landing, 'id' | 'chainId' | 'repo' | 'state' | 'createdAt'> & Partial<Omit<Landing, 'version'>>
  & { version: Pick<LandingVersion, 'number' | 'title'> & Partial<LandingVersion> }): Landing => ({
  host: 'gitlab.example.com', project: fields.repo, base: 'main', priorityWhy: '', updatedAt: fields.createdAt, rev: 1,
  ...fields,
  version: { from: '', forked: '', change: '', gates: ['reviewer', 'qc'], verdicts: [], by: [], description: '', paths: [], judged: [], ...version },
});

/** What a judge found of one item of the goal. */
export const findingRecord = (fields: Pick<Finding, 'item' | 'status' | 'evidence'> & Partial<Finding>): Finding => ({
  keyResult: '', refs: [],
  ...fields,
});

/** An event: its id, card, kind with the status of that kind (`''` for a message, a thought or a plain note) and times; its
 * words, who sent it and its data, as the case needs. */
export const eventRecord = (fields: Pick<Event, 'id' | 'home' | 'cardId' | 'createdAt' | 'updatedAt'> & EventStatus & Partial<Omit<Event, 'kind' | 'status'>>): Event =>
  ({ toolCallId: '', toolKind: '', toolName: '', title: '', text: '', rev: 1, ...fields }) as Event;

/** An event as a channel reads it: what the server types it as (a tool call, a turn's end, a block…), when it does. */
export const eventRead = (event: Event, activity?: Activity): EventRead => (activity ? { event, activity } : { event });

/** What a seed's stream writes of an event of kind `K` beside where and when it stands: that kind's status, the rest of the
 * event, and the activity the server types it as. */
export type EventSeed<K extends EventKind = EventKind> = Partial<Omit<Extract<Event, { kind: K }>, 'kind' | 'id' | 'home' | 'cardId'>> &
  Pick<EventRead, 'activity'>;

/** The status an event of each kind has when its case leaves it out: a call or a job done, a turn ended, a message read. */
const settled: { [K in EventKind]: Extract<EventStatus, { kind: K }>['status'] } = {
  message: '', thought: '', system: '', tool: 'completed', prompt: 'delivered', stop: 'end_turn', job: 'completed', task: 'completed',
};

/** An event of a seed's stream as a channel reads it: where and when it stands, its kind, and what the case writes of it. */
export const seedEvent = <K extends EventKind>(at: Pick<Event, 'id' | 'home' | 'cardId' | 'createdAt' | 'updatedAt'>, kind: K, { activity, ...fields }: EventSeed<K>): EventRead =>
  eventRead(eventRecord({ status: settled[kind], ...at, kind, ...fields } as Parameters<typeof eventRecord>[0]), activity);

/** An option of an inbox item; `kind` is a permission's (allow_once…), empty for any other item's. */
export const optionOf = (id: string, name: string, kind = ''): InboxOption => ({ id, name, kind });

/** An inbox item: its id, card, kind, title, status and time; what else it carries, as the case needs. */
export const inboxRecord = (fields: Pick<InboxItem, 'id' | 'home' | 'cardId' | 'kind' | 'title' | 'status' | 'createdAt'> & Partial<InboxItem>): InboxItem => ({
  detail: '', options: [], answer: '', source: '', connection: '', variable: '', secret: false, account: '', link: '', subscription: '',
  ownerOnly: false, rev: 1,
  ...fields,
});

/** What an error item's Retry does again: the failed turn's card and message, and the events that record the failure. */
export const retryOf = (fields: Partial<Retry>): Retry => ({ cardId: '', promptId: 0, failureIds: [], reopen: false, ...fields });

/** A permission item's call, in words: its action, why it is asked and how risky it is; the rest as the case needs. */
export const permissionAsk = (fields: Pick<PermissionAsk, 'action' | 'reason' | 'traits' | 'risk' | 'tool' | 'command'> & Partial<PermissionAsk>): PermissionAsk => ({
  said: '', decided: '',
  ...fields,
});

/** A draft: its id, home, words and time; the card it is to, as the case needs. */
export const draftRecord = (fields: Pick<Draft, 'id' | 'home' | 'text' | 'updatedAt'> & Partial<Draft>): Draft => ({
  attachments: [], cardId: '', mentor: '', rev: 1,
  ...fields,
});

/** What an agent or an employee works with: its instructions, the tools it may use (`null`: every tool) and its skills. */
export const loadoutOf = (fields: Partial<Loadout> = {}): Loadout => ({ instructions: '', tools: null, skills: [], ...fields });

/** An agent of the library: its id and name; its loadout and the rest, as the case needs. */
export const agentRecord = ({ loadout, ...fields }: Pick<Agent, 'id' | 'name'> & Partial<Omit<Agent, 'loadout'>> & { loadout?: Partial<Loadout> }): Agent => ({
  description: '', group: '', model: '', effort: '', signs: '', builtin: false, edited: false, rev: 1,
  ...fields,
  loadout: loadoutOf(loadout),
});

/** A card a playbook hires at the start: its agent, name and brief; its model and effort, as the case needs. */
export const seedRecord = (fields: Pick<Seed, 'agent' | 'name' | 'brief'> & Partial<Seed>): Seed => ({ model: '', effort: '', ...fields });

/** A playbook: its id and name; its brief, seeds and the rest, as the case needs. */
export const playbookRecord = (fields: Pick<Playbook, 'id' | 'name'> & Partial<Playbook>): Playbook => ({
  description: '', group: '', brief: '', seeds: [], builtin: false, edited: false, rev: 1,
  ...fields,
});

/** One write to a record of the library, as its history keeps it: what, by whom and when; the fields it moved, as the case
 * needs. */
export const changeRecord = (fields: Pick<Change, 'id' | 'kind' | 'name' | 'action' | 'by' | 'at'> & Partial<Change>): Change => ({
  before: '', after: '', fields: [],
  ...fields,
});

/** An employee: its id, number, template, title, status and hiring time; its loadout, ceiling and the rest, as the case needs. */
export const employeeRecord = ({ loadout, ceiling, ...fields }: Pick<Employee, 'id' | 'number' | 'template' | 'title' | 'status' | 'createdAt'>
  & Partial<Omit<Employee, 'loadout' | 'ceiling'>> & { loadout?: Partial<Loadout>; ceiling?: Partial<Ceiling> }): Employee => ({
  label: '', templateRev: 0, scope: '', strengths: '', reportsTo: '', hiredBy: '', rev: 1,
  ...fields,
  loadout: loadoutOf(loadout),
  ceiling: { effect: '', grants: [], tools: [], ...ceiling },
});

/** An employee as a channel reads it: the employee, and its words and face as the server derives them. */
export const employeeRead = (employee: Employee, more: Omit<EmployeeRead, 'employee'>): EmployeeRead => ({ employee, ...more });

/** A retrospective as a chain's screen reads it, with the name of who wrote it. */
export const retrospectiveRead = (retrospective: Retrospective, byName: string): RetrospectiveRead => ({ retrospective, byName });

/** A mentor of the pool: its id, label, status and time; its strengths, as the case needs. */
export const mentorRecord = (fields: Pick<Mentor, 'id' | 'label' | 'status' | 'createdAt'> & Partial<Mentor>): Mentor => ({ strengths: '', rev: 1, ...fields });

/** A mentor's help on a card: who, on what, its state and times; why, as the case needs. */
export const helpRecord = (fields: Pick<Help, 'id' | 'chain' | 'card' | 'mentor' | 'problem' | 'state' | 'offeredAt' | 'deadline'> & Partial<Help>): Help => ({
  why: '', rev: 1,
  ...fields,
});

/** A reading a subscription let pass, and why; what it found, as the case needs. */
export const nearMissRecord = (fields: Pick<NearMiss, 'subscriptionId' | 'seq' | 'key' | 'reason' | 'at'> & Partial<NearMiss>): NearMiss => ({
  detail: '', judgment: '',
  ...fields,
});

/** A project: its id, name, status, who wrote it and times; its customer, brief, repositories and connections, as the case
 * needs. */
export const projectRecord = (fields: Pick<Project, 'id' | 'name' | 'status' | 'by' | 'createdAt' | 'updatedAt'> & Partial<Project>): Project => ({
  customer: '', brief: '', repos: [], connections: [], rev: 1,
  ...fields,
});

/** An objective: its id, status, owner, customer and time; its outcome, horizon, key results and the rest, as the case needs. */
export const objectiveRecord = ({ horizon, ...fields }: Pick<Objective, 'id' | 'status' | 'owner' | 'customer' | 'createdAt'>
  & Partial<Omit<Objective, 'horizon'>> & { horizon?: Partial<Horizon> }): Objective => ({
  task: false, scope: '', outcome: '', northStar: '', guardrails: [], surrogates: [], unknown: [], keyResults: [], rev: 1,
  ...fields,
  horizon: { text: '', ...horizon },
});

/** A workflow run: its id, home, card, engine, name, status and time; its script, steps and the rest, as the case needs. */
export const workflowRecord = (fields: Pick<Workflow, 'id' | 'home' | 'card' | 'engine' | 'name' | 'status' | 'createdAt'> & Partial<Workflow>): Workflow => ({
  script: '', phase: '', error: '', resumedFrom: '', steps: [], rev: 1,
  ...fields,
});

/** One step of a workflow run: its run, place, verb, status and time; the rest, as the case needs. */
export const workflowStepRecord = (fields: Pick<WorkflowStep, 'workflow' | 'seq' | 'verb' | 'status' | 'at'> & Partial<WorkflowStep>): WorkflowStep => ({
  label: '', phase: '', hash: '', error: '', card: '', arrived: 0, sentBack: false,
  ...fields,
});

/** A sensor: its id, kind, name, the signal it writes, who made it and times; what it reads, as the case needs. */
export const sensorRecord = (fields: Pick<Sensor, 'id' | 'kind' | 'name' | 'signal' | 'by' | 'createdAt' | 'updatedAt'> & Partial<Sensor>): Sensor => ({
  note: '', chain: '', source: '', connection: '', query: '', everyMinutes: 0, cursor: '', paused: false, rev: 1,
  ...fields,
});

/** An event signal: its id, name, who made it and times; its note and state, as the case needs. */
export const eventSignalRecord = (fields: Pick<EventSignal, 'id' | 'name' | 'by' | 'createdAt' | 'updatedAt'> & Partial<EventSignal>): EventSignal => ({
  note: '', builtin: false, edited: false, paused: false, kept: false, rev: 1,
  ...fields,
});

/** What arrived of an event: how and its body; its headers and query, as the case needs. */
export const inboundEvent = (fields: Pick<InboundEvent, 'via' | 'body'> & Partial<InboundEvent>): InboundEvent => ({
  header: {}, query: '', bodyCut: false,
  ...fields,
});

/** A client key: its id, name, who made it and times; its limits, note and use, as the case needs. */
export const clientKeyRecord = (fields: Pick<ClientKey, 'id' | 'name' | 'by' | 'createdAt' | 'updatedAt'> & Partial<ClientKey>): ClientKey => ({
  note: '', chainsPerDay: 0, actionsPerHour: 0, paused: false, lastAddr: '', rev: 1,
  ...fields,
});

/** An agent's feedback: its id, kind, words and time; the card and agent it came from, as the case needs. */
export const feedbackRecord = (fields: Pick<Feedback, 'id' | 'kind' | 'text' | 'createdAt'> & Partial<Feedback>): Feedback => ({
  home: '', cardId: '', agent: '', name: '', model: '', effort: '', rev: 1,
  ...fields,
});

/** An account as the health check reads it: its id, kind and whether it takes work; what it said, as the case needs. */
export const accountHealth = (fields: Pick<AccountHealth, 'id' | 'kind' | 'ok'> & Partial<AccountHealth>): AccountHealth => ({
  email: '', plugin: '', connection: '', error: '', nearLimit: '',
  ...fields,
});

/** A subscription as the signal board lists it: its id, kind, title, source, what it does and where it stands; the rest,
 * as the case needs. */
export const subscriptionLine = (fields: Pick<SubscriptionLine, 'id' | 'kind' | 'title' | 'source' | 'what' | 'state' | 'group' | 'control'> & Partial<SubscriptionLine>): SubscriptionLine => ({
  chainId: '', eventSignalId: '', key: '', condition: '', screen: '', held: '', waiting: '', nearMisses: [],
  ...fields,
});

/** A signal as the board lists it: its id, key, source and title; its newest reading and who listens, as the case needs. */
export const signalLine = (fields: Pick<SignalLine, 'id' | 'key' | 'source' | 'title'> & Partial<SignalLine>): SignalLine => ({
  rev: 0, cause: '', subscribers: 0, silent: '', silentSensors: [], eventSignalId: '',
  ...fields,
});

/** A firing as a signal's page lists it: when, its title, what it did and its outcome; whose, as the case needs. */
export const firingLine = (fields: Pick<FiringLine, 'at' | 'title' | 'action' | 'outcome' | 'expired'> & Partial<FiringLine>): FiringLine => ({
  subscriptionId: '', chainId: '', cardId: '', key: '',
  ...fields,
});

/** A reading as a signal's page lists it. */
export const readingLine = (fields: Pick<ReadingLine, 'rev' | 'at' | 'cause' | 'value' | 'truncated'> & Partial<ReadingLine>): ReadingLine => ({
  eventId: '',
  ...fields,
});

/** The signal board: its lines; how many are held, as the case needs. */
export const signalBoard = (fields: Pick<SignalBoard, 'sources' | 'subscriptions' | 'signals' | 'signalsMore' | 'rev'> & Partial<SignalBoard>): SignalBoard => ({
  held: 0,
  ...fields,
});
