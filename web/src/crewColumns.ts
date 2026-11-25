// The Crew pages as boards, on each page and beside the one open: every playbook, agent, skill and tool source as a card in
// the server's columns (Boards.playbooks, .agents, .skills, .sources), one per group its record carries (the kind of work a
// playbook, agent or skill is for; the capability a source gives), narrowed by what the owner types (?q=), a page at a time.
import { computed, reactive, type ComputedRef, type WritableComputedRef } from 'vue';
import { useQueryRef } from './nav';
import type { GroupColumn, ToolSource } from './core/generated/types.generated';
import type { Standing } from './tools';

// A column as it shows: the server's column, its rows (a page of them), count, how many it holds, and hidden, how many are not
// shown.
export type CrewColumn<T> = GroupColumn & { rows: T[]; count: number; hidden: number };

const PAGE = 30;

// What the search narrows the rows to: every word has to appear in the words the page gives of a row (its name, what it is for).
export function narrowedRows<T>(rows: readonly T[], query: string, textOf: (row: T) => string): T[] {
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (!terms.length) return [...rows];
  return rows.filter((r) => {
    const words = textOf(r).toLowerCase();
    return terms.every((t) => words.includes(t));
  });
}

// The columns, each with the first `shown[key]` (or a page) of its rows in the server's order.
export function crewColumnsOf<T>(groups: readonly GroupColumn[], rows: readonly T[], groupOf: (row: T) => string, shown: Partial<Record<string, number>>): CrewColumn<T>[] {
  return groups.map((g) => {
    const all = rows.filter((r) => groupOf(r) === g.key);
    const page = all.slice(0, shown[g.key] ?? PAGE);
    return { ...g, rows: page, count: all.length, hidden: all.length - page.length };
  });
}

// What a Crew page's board is made of: the server's columns and its rows (read live from the catalog), which column a row is in,
// what names a row (its address), and the words the search looks through.
export type CrewBoard<T> = {
  groups: () => readonly GroupColumn[];
  rows: () => readonly T[];
  groupOf: (row: T) => string;
  keyOf: (row: T) => string;
  textOf: (row: T) => string;
};

export function useCrewColumns<T>(board: CrewBoard<T>): {
  columns: ComputedRef<CrewColumn<T>[]>;
  keys: ComputedRef<string[]>;
  query: WritableComputedRef<string>;
  total: ComputedRef<number>;
  more: (key: string) => void;
} {
  const query = useQueryRef('q', '');
  const shown = reactive<Partial<Record<string, number>>>({});
  const columns = computed(() => crewColumnsOf(board.groups(), narrowedRows(board.rows(), query.value, board.textOf), board.groupOf, shown));
  const more = (key: string) => (shown[key] = (shown[key] ?? PAGE) + PAGE);
  return { columns, keys: computed(() => board.groups().map((g) => g.key)), query, total: computed(() => board.rows().length), more };
}

// Playbooks, agents and skills stand in the column of their kind of work (Playbook.group, Agent.group, ListedSkill.group); sources in
// that of their capability (ToolSource.capability: the word a chain holds to get the tools).
export const workGroupOf = (r: { group: string }): string => r.group;
export const sourceGroupOf = (s: ToolSource): string => s.capability;

// How a source stands by its connections: how many can be read (ConnectionView.configured), or none can.
export function sourceStanding(s: ToolSource): Standing {
  const connected = s.connections.filter((c) => c.configured).length;
  if (!connected) return { variant: 'needs', icon: 'key', text: 'not connected' };
  return { variant: 'success', icon: 'check', text: `${connected} of ${s.connections.length} connected` };
}
