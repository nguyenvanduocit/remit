// The archived chains, which the snapshot leaves out however many the owner put away (Board.archived): read a page at a
// time as he looks at them, newest change first, and held until the snapshot is loaded again. The core reads and holds
// the pages (replica.ts); a chain pushed since (archived just now, or back on the board) is as its latest copy says, the
// catalog's or the page's.
import { computed } from 'vue';
import type { Chain, ChainRead } from './core/generated/types.generated';
import { catalog } from './catalog';
import { inOrder } from './core/client';
import { ask, coreState, useModel } from './replica';

const pages = inOrder(useModel<ChainRead>({ model: 'archived' }));

// Reads the next page: the first, or the one after the pages read.
export function readArchived(): void {
  ask({ ask: 'readArchived' });
}

// Changes each time a snapshot forgets the pages read, so what shows them reads them again, however soon after a read.
export const archivedGeneration = computed(() => coreState.value?.archivedGeneration ?? 0);

// Whether the first page has been read since the last snapshot.
export const archivedRead = computed(() => coreState.value?.archivedRead ?? false);

// Whether a page after those read is there to read.
export const archivedMore = computed(() => coreState.value?.archivedMore ?? true);

// Of two copies of one chain, the one at the higher revision is the newer.
const newer = (held: Chain | undefined, arrived: Chain): boolean => !held || arrived.rev > held.rev;

// The archived chains read so far and pushed since, newest change first.
export const archivedChains = computed<Chain[]>(() => {
  const latest = new Map<string, Chain>();
  for (const { chain: c } of [...pages.value, ...catalog.chains]) if (newer(latest.get(c.id), c)) latest.set(c.id, c);
  return [...latest.values()].filter((c) => c.archivedAt).sort((a, b) => b.updatedAt.localeCompare(a.updatedAt) || b.id.localeCompare(a.id));
});
