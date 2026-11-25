// The board as the owner sees it, on the board page and beside an open chain: the server's columns in its order, then the
// archived chains, which are on none of them (read from the server a page at a time, archived.ts). The search and the project
// narrow every column, and each shows a page of its chains at a time.
import { computed, reactive, watch, type ComputedRef, type WritableComputedRef } from 'vue';
import { archivedChains, archivedGeneration, archivedMore, archivedRead, readArchived } from './archived';
import { catalog, standingOf, chainRecords } from './catalog';
import { useProjectRef, useQueryRef } from './nav';
import type { Board, Chain, Column, InboxItem } from './core/generated/types.generated';

export type ColumnKey = Column | 'archived';
// A column as it shows: its chains and, in the first, what waits for the owner from no chain; count, how many it holds
// (on the server, for the archived chains, where only the pages read are held), hidden, how many of them are not shown, and
// empty, what it says while it holds nothing (the server's sentence: what fills it and who).
export type ShownColumn = { key: ColumnKey; label: string; empty: string; chains: Chain[]; items: InboxItem[]; count: number; hidden: number };

const PAGE = 30;

// The board's columns in its order: the server's, then the archived chains.
export const columnKeys = (board: Board | null): ColumnKey[] => [...(board?.columns ?? []).map((col) => col.key), 'archived'];

// The columns, whole: the server's, with each id it names that the replica holds, then the archived chains.
export function columnsOf(board: Board | null, chains: readonly Chain[], inbox: readonly InboxItem[], archived: readonly Chain[]): ShownColumn[] {
  const chainById = new Map(chains.map((c) => [c.id, c]));
  const itemById = new Map(inbox.map((i) => [i.id, i]));
  const pick = <T>(ids: readonly string[], by: Map<string, T>): T[] => ids.flatMap((id) => by.get(id) ?? []);
  return columnKeys(board).map((key) => {
    const col = board?.columns.find((c) => c.key === key);
    if (!col) return { key, label: 'Archived', empty: 'No chain archived: a chain you archive leaves the board and waits here.', chains: [...archived], items: [], count: board?.archived ?? 0, hidden: 0 };
    return { key, label: col.label, empty: col.empty, chains: pick(col.chains, chainById), items: pick(col.items, itemById), count: col.chains.length + col.items.length, hidden: 0 };
  });
}

// What the search narrows a column to: every word has to appear in a chain's title, who it came from or a repository it
// works in; what waits from no chain, in its title. The count is then of what matches among what is held.
export function narrowed(col: ShownColumn, query: string, fromOf: (c: Chain) => string): ShownColumn {
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (!terms.length) return col;
  const has = (text: string) => terms.every((t) => text.toLowerCase().includes(t));
  const chains = col.chains.filter((c) => has([c.title, fromOf(c), ...(c.repos ?? []).map((r) => r.repo)].join(' ')));
  const items = col.items.filter((i) => has(i.title));
  return { ...col, chains, items, count: chains.length + items.length };
}

// What narrowing the board to one project leaves of a column: the chains of that project, as the server names each chain's
// (ChainStanding.project); what waits from no chain is of no project and goes. The count is then of what is left among what is
// held, and a column left empty by it says so rather than what fills it. No project named: the column whole.
export function ofProject(col: ShownColumn, project: string, projectIdOf: (c: Chain) => string | undefined): ShownColumn {
  if (!project) return col;
  const chains = col.chains.filter((c) => projectIdOf(c) === project);
  const empty = col.count ? 'No chain of this project here.' : col.empty;
  return { ...col, chains, items: [], count: chains.length, empty };
}

// The column that holds a chain: the server's, or the archived chains for one put away. None for one the board does not
// name yet (just created, before its push).
export function columnOf(board: Board | null, chain: Pick<Chain, 'id' | 'archivedAt'>): ColumnKey | undefined {
  if (chain.archivedAt) return 'archived';
  return board?.columns.find((col) => col.chains.includes(chain.id))?.key;
}

// Who a chain came from, as the server names it, for the chains the owner did not ask for himself.
const fromOf = (c: Chain) => (c.origin && c.origin !== 'owner' ? standingOf(c.id).from : '');
// The id of a chain's project, as the server names it.
const projectOfChain = (c: Chain) => standingOf(c.id).project;

// The board for a view that shows it: the columns narrowed by the search (?q=) and to one project (?project=), both carried
// from the board into the chain a card opens and from chain to chain, each a page at a time; more shows a page more of one. The archived chains are read once a view shows
// their column (showsArchived), and again once a snapshot (a reconnect) has forgotten the pages read.
export function useBoard(showsArchived: () => boolean): { columns: ComputedRef<ShownColumn[]>; query: WritableComputedRef<string>; project: WritableComputedRef<string>; more: (key: ColumnKey) => Promise<void> } {
  const query = useQueryRef('q', '');
  const project = useProjectRef();
  const shown = reactive<Partial<Record<ColumnKey, number>>>({});
  watch([showsArchived, archivedGeneration], ([shows]) => shows && !archivedRead.value && void readArchived(), { immediate: true });
  const columns = computed(() =>
    columnsOf(catalog.board, chainRecords.value, catalog.inbox, archivedChains.value).map((whole) => {
      const col = narrowed(ofProject(whole, project.value, projectOfChain), query.value, fromOf);
      const chains = col.chains.slice(0, shown[col.key] ?? PAGE);
      return { ...col, chains, hidden: col.count - chains.length - col.items.length };
    }),
  );
  async function more(key: ColumnKey) {
    const next = (shown[key] ?? PAGE) + PAGE;
    if (key === 'archived' && archivedChains.value.length < next && archivedMore.value) await readArchived();
    shown[key] = next;
  }
  return { columns, query, project, more };
}
