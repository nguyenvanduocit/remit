// The Assistant for the verify pages, as the server would hold it (made up): a long conversation (?talk=<turns>, default
// 120, each a message from the owner with where he was, a few tool calls and an answer), a session started over in the
// middle of it, a reply from a chain's agent, the owner's newest message still waiting behind a turn that is streaming its
// answer, and an error of a turn that waits on the owner. `standing` is where it stands for each case.
import type { Actor, AgentStanding, AssistantFeed, AssistantView, Card, Decision, EventKind, EventRead, InboxItem } from '../src/core/generated/types.generated';
import { agentStands } from './standingSeed';
import { startedOver, tool, turnEnd } from './activitySeed';
import { actorOf, cardRecord, decisionRecord, inboxRecord, optionOf, seedEvent, type EventSeed } from './records';

export const ASSISTANT = 'as_1';
export const ASSISTANT_CARD = 'c_as';
const MIN = 60_000;
const OWNER: Actor = actorOf('owner', 'owner@example.com', 'web');
const iso = (ms: number) => new Date(ms).toISOString();

export function assistantCard(now: number, extra: Partial<Card> = {}): Card {
  return cardRecord({
    id: ASSISTANT_CARD, home: ASSISTANT, agent: 'assistant', name: 'Assistant', origin: 'assistant', model: 'opus', mode: 'auto', cwd: '~/.remit/assistant', status: 'running',
    sessionId: 's-as-2', plan: [], ctxUsed: 96_000, ctxSize: 200_000, turns: 121, queued: 1, activity: 'Writing…', createdAt: iso(now - 3 * 24 * 60 * MIN), updatedAt: iso(now), rev: 1, ...extra,
  });
}

// Where it stands, as the server says it: working, ready, or failed with an item that waits on the owner.
export function assistantStanding(state: 'working' | 'ready' | 'failed', waiting: string[] = []): AgentStanding {
  const asks: AgentStanding['asks'] = state === 'working' ? ['message', 'interrupt', 'restart'] : ['message', 'restart'];
  if (state === 'failed') return agentStands(ASSISTANT, ASSISTANT_CARD, 'Needs you', 'failed', 'Error: the provider refused the request', 'error', waiting, { asks, remembers: true });
  if (state === 'ready') return agentStands(ASSISTANT, ASSISTANT_CARD, 'Ready', 'idle', 'Ready', 'idle', [], { asks, remembers: true });
  return { ...agentStands(ASSISTANT, ASSISTANT_CARD, 'Working', 'running', 'Answering…', 'busy', [], { asks, remembers: true }), onMessage: undefined };
}

const PLACES = [
  { where: { page: 'board' }, label: 'the board' },
  { where: { page: 'chain', chainId: 'busy', tab: 'tree', cardId: 'eng' }, label: 'chain "Backfill missing zone groups" (busy), its tree tab, with agent Engineer — backfill zones (eng) open' },
  { where: { page: 'outcomes' }, label: 'the Outcomes page' },
  { where: { page: 'playbooks', playbookId: 'bugfix' }, label: 'playbook "Fix a bug" (bugfix) on the Playbooks page' },
];
const ASKS = ['How is the backfill going?', 'What waits on me right now?', 'Why did the migration engineer stop?', 'Pause the orchestrator split until Monday.', 'Which playbook costs the most this week?'];

// The conversation, oldest first; the last event (the answer being written) is the one that streams.
export function assistantEvents(now: number, turns: number): EventRead[] {
  let id = 900_000;
  const out: EventRead[] = [];
  const ev = <K extends EventKind>(kind: K, at: number, extra: EventSeed<K> = {}): EventRead =>
    seedEvent({ id: ++id, home: ASSISTANT, cardId: ASSISTANT_CARD, createdAt: iso(at), updatedAt: iso(at) }, kind, extra);
  const start = now - turns * 20 * MIN;
  for (let i = 0; i < turns; i++) {
    const at = start + i * 20 * MIN;
    const place = PLACES[i % PLACES.length];
    if (i === Math.floor(turns / 2)) {
      // The awkward case: the turn before ended in an error, and the owner started over after it.
      out.push(ev('stop', at - MIN - 3_000, { ...turnEnd(32_000), status: 'end_turn', data: { durationMs: 32_000 } }), ev('system', at - MIN - 2_000, { status: 'error', text: 'signal: interrupt' }));
      out.push(ev('system', at - MIN, { ...startedOver(), status: 'new_session', by: OWNER, title: 'Started over', text: 'The next message opens a new session, which reads a recap of this conversation.' }));
    }
    out.push(ev('prompt', at, { by: OWNER, status: 'delivered', title: 'Your message', text: `${ASKS[i % ASKS.length]}`, data: { placed: place } }));
    out.push(ev('tool', at + 5_000, { title: 'mcp__remit__read_board', ...tool('remit', 'read_board', 'Remit'), status: 'completed', data: { input: {}, output: '{"columns":[…]}' } }));
    out.push(ev('tool', at + 9_000, { title: 'mcp__remit__read_chain', ...tool('remit', 'read_chain', 'Remit'), status: 'completed', data: { input: { chainId: 'busy' }, output: '{"chain":{…}}' } }));
    if (i % 7 === 3) {
      out.push(ev('tool', at + 12_000, { title: 'mcp__remit__message_agent', ...tool('remit', 'message_agent', 'Remit'), status: 'completed', data: { input: { cardId: 'busy-chair', text: 'How many zones are left?' } } }));
      out.push(ev('message', at + 14_000, { text: 'I asked the Chair of Backfill missing zone groups (ch_busy) how many zones are left; its answer comes back here.' }));
      out.push(ev('stop', at + 15_000, { ...turnEnd(15_000), status: 'end_turn', data: { durationMs: 15_000 } }));
      out.push(ev('prompt', at + 4 * MIN, { by: actorOf('agent', 'busy-chair'), status: 'delivered', title: 'Reply from Chair', text: '[reply from Chair (busy-chair) in chain busy, to the message you sent it]\n3 zones are left; the engineer is on them.' }));
    }
    out.push(ev('message', at + 30_000, { text: `**${i % 2 ? 'Backfill missing zone groups' : 'Decide the retention of old events'}** (${i % 2 ? 'ch_busy' : 'ch_ask'}) ${i % 2 ? 'is running: 50 of 53 zones are done.' : 'waits on you: the Chair asks how long old events should be kept.'}\n\nNothing else waits on you.` }));
    out.push(ev('stop', at + 31_000, { ...turnEnd(31_000), status: 'end_turn', data: { durationMs: 31_000 } }));
  }
  // Now: the owner asked from the busy chain; the Assistant is reading the board, then writing; his next message waits.
  const at = now - 2 * MIN;
  out.push(ev('prompt', at, { by: OWNER, status: 'delivered', title: 'Your message', text: 'Tell the Chair of this one to have QC re-check the last 3 zones, then tell me what it says.', data: { placed: PLACES[1] } }));
  out.push(ev('tool', at + 4_000, { title: 'mcp__remit__message_agent', ...tool('remit', 'message_agent', 'Remit'), status: 'completed', data: { input: { cardId: 'busy-chair', text: 'Have QC re-check the last 3 zones.' } } }));
  out.push(ev('prompt', now - MIN, { by: OWNER, status: 'queued', title: 'Your message', text: 'And archive the two done chains when that is sent.', data: { placed: { where: { page: 'board' }, label: 'the board' } } }));
  out.push(ev('message', now - 30_000, { text: 'I told the Chair of **Backfill missing zone groups**' }));
  return out;
}

// A turn of the Assistant that failed and waits on the owner, as its inbox item.
export const assistantError = (now: number): InboxItem => inboxRecord({
  id: 'i_as', home: ASSISTANT, cardId: ASSISTANT_CARD, kind: 'error', title: 'Assistant stopped', detail: 'the provider refused the request: overloaded', options: [optionOf('retry', 'Retry'), optionOf('stop', 'Leave it')],
  status: 'open', createdAt: iso(now - MIN), rev: 1,
});

export function assistantView(now: number, card: Card, agent: AgentStanding): AssistantView {
  return { assistant: { id: ASSISTANT, cardId: ASSISTANT_CARD, createdAt: card.createdAt }, card, agent, drafts: [] };
}

// What the Assistant decided for the owner, newest first, as the server lays it out (orch/assist_feed.rs): an irreversible grant
// it applied that waits out its overturn window (ADR-0101), an answer it gave for him that he overturned, one that stands (he may
// overturn it) with a second judge's dissent (ADR-0103), an item it left to him, and an older answer; `unseen` of them
// came since he last looked.
export function assistantFeedSeed(now: number, unseen = 2): AssistantFeed {
  const by: Actor = actorOf('assistant', 'as');
  const d = (id: string, minAgo: number, text: string, why: string, item: string): Decision =>
    decisionRecord({ id, chainId: 'busy', kind: 'assist', by, text, why, item, createdAt: iso(now - minAgo * MIN) });
  return {
    unseen,
    lines: [
      { answered: true, takesEffectAt: iso(now + 52 * MIN), decision: d('d_as4', 8, 'Decided for you: Support asks to write grant ch_support/intercom/default → apply, taking effect at ' + new Date(now + 52 * MIN).toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' }) + ' unless you overturn it', 'You asked for the overdue-invoice replies to go out from Intercom this week; the grant covers that one connection.', 'i_grant') },
      { answered: true, decision: d('d_as3', 4, 'Decided for you: Engineer asks to run `rm -rf /tmp/backfill-cache` → allow, once', 'The cache is rebuilt on the next run and the path is inside the chain\'s worktree; you allowed the same clean-up twice last week.', 'i_rm'),
        dissent: decisionRecord({ id: 'd_dis', chainId: 'busy', kind: 'dissent', by: actorOf('agent', 'c_so'), text: 'A second judge decides otherwise: Engineer asks to run `rm -rf /tmp/backfill-cache` → Refuse', why: 'The backfill reads that cache until it finishes, and the run is half done: removing it now restarts six hours of work.\n\nHow they differ: one lets the clean-up run, the other holds it until the backfill ends.', item: 'i_rm', createdAt: iso(now - 2 * MIN) }) },
      { answered: true, decision: d('d_as2', 18, 'Decided for you: Release asks to push to origin/main → refuse: let the merge queue land it instead', 'You never let a chain push to main; the merge queue lands every approved change.', 'i_push'),
        overturn: decisionRecord({ id: 'd_ov1', chainId: 'busy', kind: 'overturn', by: OWNER, text: 'Overturned: Release asks to push to origin/main → refuse: let the merge queue land it instead', why: 'Allow it this once: the hotfix is already reviewed in #ops and the MR queue is frozen.', item: 'i_push', overturns: 'd_as2', createdAt: iso(now - 9 * MIN) }) },
      { answered: false, decision: d('d_as1', 55, 'Left to you: Grafana needs a service-account token', 'A credential is yours to type: I never see one.', 'i_cred') },
      { answered: true, decision: d('d_as0', 26 * 60, 'Decided for you: The Chair asks whether the backfill may run tonight → yes, after 22:00', 'Your standing rule: heavy jobs run after 22:00 local time.', 'i_tonight') },
    ],
  };
}
