// What waits for the owner, as the server lists it (Board.waiting, ADR-0078): one list in the one order every surface shows,
// read here by the board, the top bar, the palette, the keys and the notifications. Nothing here decides what waits or in
// what order; this module only says where each wait is answered.
import { computed } from 'vue';
import { useRouter, type RouteLocationRaw } from 'vue-router';
import { catalog, listedChain } from './catalog';
import { useAssistant } from './nav';
import type { Wait } from './core/generated/types.generated';

const NONE: readonly Wait[] = [];
export const waiting = computed((): readonly Wait[] => catalog.board?.waiting ?? NONE);

// When the oldest of what waits began, for the column that holds them; none when nothing waits.
export const oldestSince = computed(() => waiting.value.reduce<string | undefined>((at, w) => (!at || w.since < at ? w.since : at), undefined));

// Where a wait is answered: what an agent asks beside that agent, in its drawer on the chain's Live tab (the Chair's when it
// is the Chair's); what waits from a chain and no agent on the chain's Overview; what an agent in no chain asks, the Assistant
// being the only one, in its panel ('assistant'); what waits from no agent and no chain on the board, in its first column.
export function waitTarget(w: Wait): RouteLocationRaw | 'assistant' {
  if (!w.chainId) return w.cardId ? 'assistant' : { name: 'board' };
  const id = w.chainId;
  if (!w.cardId) return { name: 'chain', params: { id, tab: 'overview' } };
  const chair = listedChain(id)?.chairId;
  return { name: 'chain', params: { id, tab: 'live' }, query: { card: w.cardId === chair ? undefined : w.cardId } };
}

// Opens where a wait is answered (waitTarget): the palette's row, the `n` key and a notification's click go there alike.
export function useOpenWait(): (w: Wait) => void {
  const router = useRouter();
  const { show } = useAssistant();
  return (w) => {
    const to = waitTarget(w);
    if (to === 'assistant') show();
    else void router.push(to);
  };
}
