// Who works under whom, as the server lays it out (AgentStanding.children, and the Chair's peers): the Chair at the root,
// the cards Remit started (the judges) beside it, each agent of the team under the one whose work it builds on.
// Nothing here decides a place; this nests the server's answer and lists it as the outline draws it.
import type { AgentStanding, Card, ChainView } from './core/generated/types.generated';

export interface TreeNode {
  card: Card;
  agent: AgentStanding;
  children: TreeNode[];
  /** The other earlier agents it works with, besides the one it hangs under. */
  with: Card[];
  /** The card whose process it runs inside, when Claude Code spawned it there. */
  inside?: Card;
}

export interface TreeRoots {
  chair: TreeNode;
  peers: TreeNode[];
}

export function treeOf(view: ChainView): TreeRoots | null {
  const cards = new Map(view.cards.map((c) => [c.id, c]));
  const agents = new Map(view.agents.map((a) => [a.cardId, a]));
  // A card the server has not stood yet (just hired, before its push) has no place yet.
  const node = (id: string): TreeNode | null => {
    const card = cards.get(id);
    const agent = agents.get(id);
    if (!card || !agent) return null;
    const inside = card.spawn && cards.get(card.spawn.parent);
    return { card, agent, children: nodes(agent.children), with: agent.with.flatMap((w) => cards.get(w) ?? []), ...(inside && { inside }) };
  };
  const nodes = (ids: readonly string[]): TreeNode[] => ids.map(node).filter((n): n is TreeNode => n !== null);
  const chair = node(view.chain.chain.chairId);
  return chair && { chair, peers: nodes(chair.agent.peers) };
}

/** One line of the outline: an agent, how far in it stands, and the row of the one it hangs from (-1 for the Chair). */
export interface Row {
  node: TreeNode;
  level: number;
  parent: number;
}

// The Chair and its team as the outline lists them, top to bottom, each agent under the one it hangs from and nothing under
// a folded one. The Chair's agents step in one level; under an agent of the team, an only child goes on at its parent's
// level, so a run of work handed on from one agent to the next reads as one straight column, and siblings step in one. A card
// Claude Code spawned is not handed work on: it runs inside its parent, so it always steps in under it.
export function outline(roots: TreeRoots, folded: ReadonlySet<string>): Row[] {
  const rows: Row[] = [];
  const add = (node: TreeNode, level: number, parent: number) => {
    const at = rows.push({ node, level, parent }) - 1;
    if (folded.has(node.card.id)) return;
    const next = parent >= 0 && node.children.length === 1 && !node.children[0].card.spawn ? level : level + 1;
    node.children.forEach((k) => add(k, next, at));
  };
  add(roots.chair, 0, -1);
  return rows;
}

// The outline's geometry: every row has the same height, so its lines follow from the rows alone. An event's steps
// place their marks and their line on the same grid (TreeRow, EventFlow), so an agent reads alike in both.
/** The height of a row. */
export const ROW = 36;
/** How far a level stands in from the one before it. */
export const INDENT = 20;
/** The size of an agent's badge in a row, and how far the first one stands from the row's left edge. */
export const ROW_BADGE = 20;
export const ROW_PAD = 8;

const badgeX = (level: number) => ROW_PAD + level * INDENT + ROW_BADGE / 2;

/** The outline's lines: from the badge of the row each row hangs from, straight down to a row that goes on at its level, else down and across to its badge. A row stepped in that hangs from no row (-1) hangs from what stands above the outline (an event's step), from its top edge. */
export function elbows(rows: readonly { level: number; parent: number }[]): string[] {
  const out: string[] = [];
  rows.forEach((r, i) => {
    if (r.parent < 0) {
      if (r.level > 0) out.push(`M${badgeX(r.level - 1)},0 V${i * ROW + ROW / 2} H${badgeX(r.level) - ROW_BADGE / 2}`);
      return;
    }
    const p = rows[r.parent];
    const x = badgeX(p.level);
    const from = r.parent * ROW + (ROW + ROW_BADGE) / 2;
    out.push(p.level === r.level ? `M${x},${from} V${i * ROW + (ROW - ROW_BADGE) / 2}` : `M${x},${from} V${i * ROW + ROW / 2} H${badgeX(r.level) - ROW_BADGE / 2}`);
  });
  return out;
}

// An agent at work: its time keeps counting.
export const working = (c: Card): boolean => c.status === 'running' || c.status === 'starting' || c.status === 'permission';
