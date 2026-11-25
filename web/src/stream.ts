// Turns a card's events into what the stream shows: consecutive tool calls fold into one summary line
// ("read 3 files, ran 2 commands"), thinking is its own block, and each turn ends in its row: how long it took and, as the
// server tells it, what it touched and what failed.
import type { Activity, Actor, ActorKind, Block as Shown, Decision, EventData, EventRead, InboxItem, Judge, OnMessage, RetrospectiveRead, ToolGroup, TurnSummary } from './core/generated/types.generated';
import { clientKeyNameOf } from './catalog';
import { mentorOfHelp } from './mentors';
import { plural, type Look, type Tone } from './ui';

export type { ToolGroup };

type ToolActivity = Extract<Activity, { type: 'tool' }>;
type SentActivity = Extract<Activity, { type: 'sent' }>;

export type Block =
  | { type: 'prompt'; key: string; event: EventRead }
  | { type: 'message'; key: string; event: EventRead }
  | { type: 'thought'; key: string; event: EventRead }
  | { type: 'tools'; key: string; events: EventRead[]; summary: string; running: boolean; failed: number } // `failed`: its calls the server typed failed
  | { type: 'turnEnd'; key: string; event: EventRead; ms?: number; turn?: TurnSummary } // `ms`: how long the turn took, `turn` its row, each when the server recorded it
  | { type: 'block'; key: string; event: EventRead; block: Shown } // what the agent showed with `show_block` (ADR-0091), its picks as they stand
  | { type: 'work'; key: string; event: EventRead } // a job the server runs, or a task the agent's process runs, outside a turn
  | { type: 'asked'; key: string; event: EventRead; request: string } // a permission prompt not decided yet: see permissionLabel
  | { type: 'decision'; key: string; event: EventRead; request: string } // a permission prompt decided: see permissionLabel
  | { type: 'system'; key: string; event: EventRead }
  | { type: 'instructions'; key: string; events: EventRead[] } // instructions files the session read one after another: each titled by its path
  | ({ type: 'sent'; key: string; event: EventRead } & Omit<SentActivity, 'type'>) // `to` is a card id, none for the Chair
  | { type: 'ask'; key: string; item: InboxItem } // what the inbox asks of the owner, where it was raised
  | { type: 'verdict'; key: string; judge: Judge }; // one round a judge gave, where it was given: `judge.latest` is that round

// A tool call as the server typed it (Activity `tool`): its group, the name and words it is shown by, where it stands.
// An event the server sent untyped is shown as a tool of no group.
const UNTYPED: ToolActivity = { type: 'tool', group: 'other', name: 'tool', summary: '', status: 'running' };
export function toolOf(e: EventRead): ToolActivity {
  return e.activity?.type === 'tool' ? e.activity : { ...UNTYPED, summary: e.event.title };
}

// What an event's data holds that the page reads (a tool call's input and output, a background task's, where the owner
// was): the server writes it as `EventData` says (`chains::activity` reads it so too).
export const dataOf = (e: EventRead): EventData => (e.event.data ?? {}) as EventData;

// A tool group's mark.
export const TOOL_ICON: Record<ToolGroup, string> = { read: 'file', edit: 'pencil', execute: 'terminal', search: 'search', fetch: 'download', remit: 'link', other: 'gear' };

const PHRASE: Record<ToolGroup, (n: number) => string> = {
  read: (n) => `read ${plural(n, 'file')}`,
  edit: (n) => `edited ${plural(n, 'file')}`,
  execute: (n) => `ran ${plural(n, 'command')}`,
  search: (n) => `searched ${plural(n, 'time')}`,
  fetch: (n) => `fetched ${plural(n, 'page')}`,
  remit: (n) => `called Remit ${plural(n, 'time')}`,
  other: (n) => `used ${plural(n, 'other tool')}`,
};
const ORDER: ToolGroup[] = ['edit', 'execute', 'read', 'search', 'fetch', 'remit', 'other'];

// The words for a group of tool calls, counted by the group the server gave each.
export function toolSummary(events: EventRead[]): string {
  const counts = new Map<ToolGroup, number>();
  for (const e of events) counts.set(toolOf(e).group, (counts.get(toolOf(e).group) ?? 0) + 1);
  return ORDER.flatMap((g) => {
    const n = counts.get(g);
    return n ? [PHRASE[g](n)] : [];
  }).join(', ');
}

const isRunning = (e: EventRead) => toolOf(e).status === 'running';
const isFailed = (e: EventRead) => toolOf(e).status === 'failed';

// blocks expects one card's events, oldest first. `asks` are the items waiting on the owner from that card and `verdicts` the
// rounds it judged (one Judge each, its `latest` the round): each sits before the first event that came after it, so it reads
// in the order things happened; an event of the same instant is what raised it.
export function blocks(events: readonly EventRead[], asks: readonly InboxItem[] = [], verdicts: readonly Judge[] = []): Block[] {
  const out: Block[] = [];
  let tools: EventRead[] = [];
  const flush = () => {
    if (!tools.length) return;
    out.push({ type: 'tools', key: 'g' + tools[0].event.id, events: tools, summary: toolSummary(tools), running: tools.some(isRunning), failed: tools.filter(isFailed).length });
    tools = [];
  };
  const raised = [
    ...asks.map((item): { at: number; block: Block } => ({ at: Date.parse(item.createdAt), block: { type: 'ask', key: item.id, item } })),
    ...verdicts.map((judge): { at: number; block: Block } => ({ at: Date.parse(judge.latest.at), block: { type: 'verdict', key: 'v' + judge.latest.id, judge } })),
  ].sort((a, b) => a.at - b.at);
  const said = new Set(asks.flatMap((item) => item.retry?.failureIds ?? [])); // a failure an ask says, it alone shows
  // A permission prompt decided shows as its decision alone: the decision names who decided and what was asked.
  const decided = new Set(events.flatMap((e) => (e.activity?.type === 'decided' && e.activity.id ? [e.activity.id] : [])));
  const raisedBefore = (at: number) => {
    for (; raised.length && raised[0].at < at; raised.shift()) {
      flush();
      out.push(raised[0].block);
    }
  };
  for (const e of events) {
    raisedBefore(Date.parse(e.event.createdAt));
    if (said.has(e.event.id)) continue;
    if (e.event.kind === 'tool') {
      if (e.activity?.type !== 'sent') {
        tools.push(e);
        continue;
      }
      flush();
      const { type: _, ...sent } = e.activity;
      out.push({ type: 'sent', key: 's' + e.event.id, event: e, ...sent });
      continue;
    }
    flush();
    const key = String(e.event.id);
    switch (e.event.kind) {
      case 'prompt':
        out.push({ type: 'prompt', key, event: e });
        break;
      case 'message':
        out.push({ type: 'message', key, event: e });
        break;
      case 'thought':
        out.push({ type: 'thought', key, event: e });
        break;
      case 'job':
      case 'task':
        out.push({ type: 'work', key, event: e });
        break;
      case 'stop': {
        const end = e.activity?.type === 'turn_end' ? e.activity : undefined;
        out.push({ type: 'turnEnd', key, event: e, ms: end?.ms, turn: end?.turn });
        break;
      }
      default:
        if (e.activity?.type === 'block') out.push({ type: 'block', key, event: e, block: e.activity.block });
        else if (e.activity?.type === 'decided') out.push({ type: 'decision', key, event: e, request: e.activity.request });
        else if (e.activity?.type === 'asked') {
          if (!decided.has(e.activity.id)) out.push({ type: 'asked', key, event: e, request: e.activity.request });
        } else if (e.event.status === 'instructions_read') {
          const last = out.at(-1);
          if (last?.type === 'instructions') last.events.push(e);
          else out.push({ type: 'instructions', key, events: [e] });
        } else out.push({ type: 'system', key, event: e });
    }
  }
  flush();
  raisedBefore(Infinity);
  return out;
}

// Only what the owner sent, himself or through the Assistant, is his words in the stream (a chat bubble), by who sent it
// (every event carries its actor).
export const ownerSent = (e: EventRead): boolean => e.event.by?.kind === 'owner' || e.event.by?.kind === 'assistant';

// A row of the agent working its way to what it says next (a tool call, a thought, a decision, a note), as against what
// it says, what the owner sent or must see, a message between it and another agent, a block it showed, and where a turn or
// session ends.
export function isStep(b: Block): boolean {
  switch (b.type) {
    case 'tools':
    case 'thought':
    case 'work':
    case 'asked':
    case 'decision':
    case 'instructions':
      return true;
    case 'prompt':
      return !ownerSent(b.event) && !fromAgent(b.event) && !mentorSaid(b.event);
    case 'system':
      return b.event.event.status !== 'error' && b.event.event.status !== 'new_session';
    default:
      return false;
  }
}

// What a mentor said to the session of the card they help (ADR-0062): their words, as against the news of the session's own
// background work.
export const mentorSaid = (e: EventRead): boolean => e.event.by?.kind === 'mentor';

// A mentor's message without the header line the server put over it for the agent ("[from Lan, your mentor]").
export const mentorText = (e: EventRead): string => e.event.text.replace(/^\[from [^\n]*, your mentor\]\n/, '');

// More rows of steps than this between two messages fold into one line the owner opens, so a finished stretch of work
// scans as what the agent said.
const FOLD_OVER = 5;
export interface StepRun {
  key: string; // the first row's, so a run keeps it as the stream grows
  start: number; // index of its first block
  end: number; // index past its last
}
// foldedRuns finds the runs of steps that a message closes and that are long enough to fold. The run after the last
// message stays open: the agent is still at it.
export function foldedRuns(bs: readonly Block[]): StepRun[] {
  const out: StepRun[] = [];
  let start = -1;
  bs.forEach((b, i) => {
    if (isStep(b)) {
      if (start < 0) start = i;
      return;
    }
    if (b.type === 'message' && start >= 0 && i - start > FOLD_OVER) out.push({ key: 'f' + bs[start].key, start, end: i });
    start = -1;
  });
  return out;
}

// Where one step of an agent's work stands, as its mark shows it (StandingMark): a tool call, a job or task it ran, a step of
// its plan. Done, failed and stopped each have their icon; any other status is still at work and shows the ring.
const STEP: Record<string, Look & { label: string }> = {
  completed: { tone: 'success', icon: 'check', label: 'Done' },
  failed: { tone: 'danger', icon: 'alert', label: 'Failed' },
  stopped: { tone: 'quiet', icon: 'stop', label: 'Stopped' },
};
export function stepLook(status: string | undefined): Look & { label: string; working: boolean } {
  const ended = STEP[status ?? ''];
  return ended ? { ...ended, working: false } : { tone: 'info', label: 'Running', working: true };
}

// Live feed: runs of consecutive events from the same card, each folded into blocks. A run whose events are the very
// records of a run in `previous` keeps that run's blocks, so what shows them is not drawn again (records are replaced,
// never changed, when an event changes: see openChain.ts).
export interface Segment {
  cardId: string;
  key: string;
  events: EventRead[];
  blocks: Block[];
}
export function segments(events: readonly EventRead[], previous: readonly Segment[] = []): Segment[] {
  const runs: Omit<Segment, 'blocks'>[] = [];
  for (const e of events) {
    const last = runs[runs.length - 1];
    if (last && last.cardId === e.event.cardId) last.events.push(e);
    else runs.push({ cardId: e.event.cardId, key: 's' + e.event.id, events: [e] });
  }
  const was = new Map(previous.map((s) => [s.key, s]));
  return runs.map((r) => {
    const p = was.get(r.key);
    const same = p && p.events.length === r.events.length && p.events.every((e, i) => e === r.events[i]);
    return { ...r, blocks: same ? p.blocks : blocks(r.events) };
  });
}

// The chain's Live feed in the order things happened: runs of one agent's work, and between them each outcome of the chain
// (a decision the server marks `outcome`: merged, paused, archived, with who and why) and each retrospective the coach wrote. A mark sits
// before the first run that began after it, so one made during a run follows that run.
export type FeedItem =
  | { type: 'run'; key: string; run: Segment }
  | { type: 'outcome'; key: string; decision: Decision }
  | { type: 'retro'; key: string; retrospective: RetrospectiveRead };
export function feed(runs: readonly Segment[], outcomes: readonly Decision[], retrospectives: readonly RetrospectiveRead[]): FeedItem[] {
  const marks = [
    ...outcomes.map((decision): { at: number; item: FeedItem } => ({ at: Date.parse(decision.createdAt), item: { type: 'outcome', key: 'o' + decision.id, decision } })),
    ...retrospectives.map((retrospective): { at: number; item: FeedItem } => ({ at: Date.parse(retrospective.retrospective.createdAt), item: { type: 'retro', key: 'r' + retrospective.retrospective.id, retrospective } })),
  ].sort((a, b) => a.at - b.at);
  const out: FeedItem[] = [];
  let m = 0;
  for (const run of runs) {
    const began = Date.parse(run.events[0].event.createdAt);
    for (; m < marks.length && marks[m].at < began; m++) out.push(marks[m].item);
    out.push({ type: 'run', key: run.key, run });
  }
  for (; m < marks.length; m++) out.push(marks[m].item);
  return out;
}

// A tool call as its card shows it: the name and words the server gave it, and how long it took.
export function toolCall(e: EventRead, now = Date.now()): { name: string; summary: string; ms: number; running: boolean } {
  const { name, summary } = toolOf(e);
  const running = isRunning(e);
  const ms = (running ? now : new Date(e.event.updatedAt).getTime()) - new Date(e.event.createdAt).getTime();
  return { name, summary, ms, running };
}

// Whether a permission prompt decided was allowed, as the server typed it.
const allowed = (e: EventRead): boolean => e.activity?.type === 'decided' && e.activity.allowed;

// Where a permission prompt of the card stands, as the owner reads it: who it waits on (him or the Chair), or who decided it
// and how: its lead (the Chair), he himself, the Assistant for him, the session itself, a rule, or the policy that decided once.
export function permissionLabel(e: EventRead): string {
  if (e.activity?.type === 'asked') return e.activity.of === 'owner' ? 'Asks you' : 'Asks the Chair';
  const verb = allowed(e) ? 'allowed' : 'refused';
  const by = e.event.by;
  switch (by?.kind) {
    case 'owner':
      return `You ${verb}`;
    case 'assistant':
      return `The Assistant ${verb} for you`;
    case 'remit':
      if (by.who === 'policy') return `Policy ${verb}`;
      return allowed(e) ? 'Allowed by a rule' : 'Refused by a rule';
    case 'agent':
      if (by.who === e.event.cardId) return `The session ${verb} itself`;
  }
  return `Chair ${verb}`;
}

// A system event that is not an error is a note in the stream: a permission prompt asked or who decided it (allowed, refused), that the work was handed over to another seat, that Remit moved the card to another provider or has it wait
// for one, who ended the turn, or what Claude Code did to the session beside the agent's work (a request retried, the
// conversation compacted, the model switched, a call or a settings change a rail refused).
// Its icon and tone say which, the event's text says the rest.
export function noteOf(e: EventRead): Required<Look> {
  const { status } = e.event;
  if (e.activity?.type === 'decided') return allowed(e) ? { icon: 'check', tone: 'success' } : { icon: 'x', tone: 'danger' };
  if (status === 'refused') return { icon: 'x', tone: 'danger' };
  if (status === 'retrying') return { icon: 'repeat', tone: 'warning' };
  if (status === 'model_switched') return { icon: 'repeat', tone: 'warning' };
  if (status === 'compacted') return { icon: 'archive', tone: 'quiet' };
  if (status === 'handed_over') return { icon: 'arrow-right', tone: 'info' };
  if (e.activity?.type === 'asked') return { icon: 'clock', tone: 'quiet' };
  if (status === 'moved') return { icon: 'arrow-right', tone: 'quiet' };
  if (status === 'waiting') return { icon: 'clock', tone: 'quiet' };
  return { icon: 'interrupt', tone: 'quiet' };
}

// The agent that sent a prompt: its card id; undefined when the owner or Remit sent it.
export const agentSender = (e: EventRead): string | undefined => (e.event.by?.kind === 'agent' ? e.event.by.who : undefined);

// A prompt another agent sent, as against the news of the agent's own background work (its own card is the sender).
export const fromAgent = (e: EventRead): boolean => agentSender(e) !== undefined && agentSender(e) !== e.event.cardId;

// Who did something, as the owner reads it: he did, the Assistant did it for him, an agent (named by `nameOf`), a mentor
// (named through the help they act on), Remit did, or an outside agent through a client key (named by the key, "deleted key <id>" once it is gone).
const ACTOR: Record<ActorKind, (by: Actor, nameOf: (cardId: string) => string) => string> = {
  owner: () => 'You',
  assistant: () => 'Assistant · for you',
  agent: (by, nameOf) => (by.who ? nameOf(by.who) : 'An agent'),
  mentor: (by) => mentorOfHelp(by.who),
  remit: () => 'Remit',
  client: (by) => `Client · ${clientKeyNameOf(by.who)}`,
};
export const actorName = (by: Actor, nameOf: (cardId: string) => string): string => ACTOR[by.kind](by, nameOf);
// An actor on a record the owner keeps beyond any card (an event signal): an agent shows as the id it carries.
export const actorLabel = (by: Actor): string => actorName(by, (id) => id);

// Where a message put to an agent stands: has it arrived, is it being read, was it read. `onMessage` is the message the
// server says the agent is on (AgentStanding.onMessage), while it is in a turn; any other message it took, it has read.
// `queued`: the message waits for the agent's current work, which the owner may end so it is read now.
export interface Receipt {
  text: string;
  tone: Tone;
  queued: boolean;
}

export function receipt(e: EventRead, onMessage: OnMessage | undefined): Receipt {
  if (e.event.status === 'queued') return { text: 'Sent · waiting for the current work to finish', tone: 'warning', queued: true };
  if (e.event.status === 'dropped') return { text: 'Not read — the session closed first', tone: 'danger', queued: false };
  if (onMessage?.eventId === e.event.id) return { text: onMessage.answering ? 'Processing…' : 'Reading…', tone: 'info', queued: false };
  return { text: 'Read', tone: 'success', queued: false };
}
