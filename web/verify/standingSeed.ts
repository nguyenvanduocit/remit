// Where things stand, as the server says it (orch/standing.go), for the verify pages: made-up values a page writes out or
// makes up for its seeds. The rules that lay them out are the server's alone; nothing in src computes them.
import type { AgentStanding, AgentTone, Board, Card, ChainStanding, ChainTone, Column, DoingTone, InboxItem, InboxKind, Wait, WaitKind } from '../src/core/generated/types.generated';

let rev = 1;
export const stands = (chainId: string, text: string, tone: ChainTone, extra: Partial<ChainStanding> = {}): ChainStanding => ({
  chainId, text, tone, working: false, waits: 0, unseen: false, asks: [], from: 'you', rev: rev++, ...extra,
});

// What waits for the owner, as the server words it (orch waiting.rs, ADR-0078): an inbox item's kind and its word.
const KIND: Record<InboxKind, WaitKind> = { question: 'asks', permission: 'approve', error: 'failed', change: 'apply', credential: 'key', login: 'sign_in', exception: 'decide', held: 'held' };
export const WAIT_LABEL: Record<WaitKind, string> = { asks: 'Asks', approve: 'Approve', failed: 'Failed', apply: 'Apply', key: 'Key', sign_in: 'Sign in', decide: 'Decide', held: 'Held' };
// `chain`: whether the item's home is a chain (else the Assistant's, or a chain since deleted).
export const itemWait = (i: InboxItem, who: string, chain = true, extra: Partial<Wait> = {}): Wait => ({
  id: i.id, kind: KIND[i.kind], label: WAIT_LABEL[KIND[i.kind]], chainId: chain ? i.home : undefined, cardId: i.cardId || undefined,
  who, ask: i.title, since: i.createdAt, unseen: false, ...extra,
});
// A chain that needs the owner stands as the server says it: "Needs you", in red when its first wait failed, its first wait and
// how many of the list are its own.
export const needs = (chainId: string, waits: Wait[], extra: Partial<ChainStanding> = {}): ChainStanding => {
  const mine = waits.filter((w) => w.chainId === chainId);
  return stands(chainId, 'Needs you', mine[0]?.kind === 'failed' ? 'failed' : 'needs', { wait: mine[0], waits: mine.length, ...extra });
};

// What a stop does, as the server words it (lifecycle::stop_says): an agent of a running chain, whose Chair is told, and one no
// Chair is told of (the Assistant, in no chain), and the Chair itself.
export const STOP_TELLS_CHAIR = 'Its session ends for good and the messages waiting for it are dropped. The Chair reads your why and is told not to restart it on its own.';
export const STOP_NO_CHAIR = 'Its session ends for good and the messages waiting for it are dropped. No Chair is told: your why stays in its stream.';
export const STOP_CHAIR = 'Its session ends for good and the messages waiting for it are dropped. It is the Chair, so no one is told: your why stays in its stream.';

// An agent's standing (stopSays: the server's words when asks holds stop, empty otherwise); extra is its place in the tree (children, with, peers, branch), a leaf unless given, what the owner can
// ask of it (asks), a message unless given, and whether it has a memory (remembers), when it is an employee's card.
export const agentStands = (chainId: string, cardId: string, label: string, tone: AgentTone, doing: string, doingTone: DoingTone, waiting: string[] = [],
  extra: Partial<Pick<AgentStanding, 'children' | 'with' | 'peers' | 'branch' | 'asks' | 'remembers' | 'work' | 'since' | 'warms' | 'stopSays'>> = {}): AgentStanding => ({
  cardId, home: chainId, label, tone, doing, doingTone, work: '', warms: tone === 'running', waiting, children: [], with: [], peers: [], branch: { agents: 0, turns: 0, working: false }, asks: ['message'], remembers: false,
  stopSays: extra.asks?.includes('stop') ? STOP_TELLS_CHAIR : '', ...extra, rev: rev++,
});

// What the server says of each column while it holds nothing (crates/orch/src/standing.rs COLUMNS, worker.rs MENTOR_COLUMNS).
const EMPTY: Record<Column, string> = {
  needs: 'What only you can settle lands here: a sign-in code, a credential, a key to read. The Assistant decides the rest.',
  running: 'A chain stands here while its Chair and team work on it, and while you have it paused.',
  waiting: 'A chain stands here while its Chair waits to check back, until its time comes or a message wakes it.',
  watching: 'A chain stands here once its change is merged, while its Chair watches a signal until the effect is verified.',
  queued: 'A new chain waits here for room in the limit: key results with nothing running first, then the soonest horizon, then the oldest; one held says why.',
  finished: 'A chain lands here once its Chair or you finish it, until it is archived.',
  asked: 'A Chair that asks for your help puts its chain here until you accept or decline.',
  helping: 'A chain stands here while you help it, from the help you accepted until it ends.',
};
const MENTOR_DONE = 'A help lands here once it ends or you decline it.';
const LABEL: Record<Column, string> = { asked: 'Asked', helping: 'Helping', needs: 'Needs you', running: 'Running', waiting: 'Waiting', watching: 'Watching', queued: 'Queue', finished: 'Finished' };
// The owner's board unless keys names a mentor's columns (asked, helping, finished).
export function boardSeed(columns: Partial<Record<Column, string[]>>, items: string[] = [], archived = 0, waiting: Wait[] = [], keys: Column[] = ['needs', 'running', 'waiting', 'watching', 'queued', 'finished']): Board {
  return {
    columns: keys.map((key) => ({ key, label: LABEL[key], empty: key === 'finished' && keys.includes('asked') ? MENTOR_DONE : EMPTY[key], chains: columns[key] ?? [], items: key === 'needs' ? items : [] })),
    archived, needsYou: (columns.needs?.length ?? 0) + items.length, waiting, rev: rev++,
  };
}

// Places agents in the tree as the server would (made up): parentOf names whom each agent of the team hangs under, the
// Chair when it names no one, and withOf the other agents it works with; the cards Remit started (origin judge) stand
// beside the Chair. Cards oldest first.
export function placed(chairId: string, cards: Card[], agents: AgentStanding[], parentOf: Record<string, string> = {}, withOf: Record<string, string[]> = {}): AgentStanding[] {
  const born = [...cards].sort((a, b) => a.createdAt.localeCompare(b.createdAt));
  const kids = (id: string) => born.filter((k) => k.origin !== 'chair' && k.origin !== 'judge' && (parentOf[k.id] ?? chairId) === id).map((k) => k.id);
  const branch = (id: string): AgentStanding['branch'] => {
    const k = cards.find((x) => x.id === id)!;
    const own = { agents: 0, turns: k.turns, working: k.status === 'running' || k.status === 'starting' || k.status === 'permission' };
    return kids(id).map(branch).reduce((b, x) => ({ agents: b.agents + 1 + x.agents, turns: b.turns + x.turns, working: b.working || x.working }), own);
  };
  return agents.map((a) => ({ ...a, children: kids(a.cardId), with: withOf[a.cardId] ?? [], peers: a.cardId === chairId ? born.filter((k) => k.origin === 'judge').map((k) => k.id) : [], branch: branch(a.cardId) }));
}
