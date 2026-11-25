// What came in, as a list on the Events page and beside an open event: every event newest first (catalog.events, the server's
// newest 200), narrowed by what the owner types (?q=), a page at a time.
import { computed, ref, type ComputedRef, type WritableComputedRef } from 'vue';
import { catalog } from './catalog';
import { useQueryRef } from './nav';
import type { EventRecord } from './core/generated/types.generated';

const PAGE = 30;

// What the search narrows the events to: every word has to appear in the signal it came through, the way it came, or a firing's
// subscription or outcome.
export function narrowedEvents(rows: readonly EventRecord[], query: string): EventRecord[] {
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (!terms.length) return [...rows];
  return rows.filter((e) => {
    const text = [e.signal, e.via, ...e.firings.flatMap((f) => [f.title, f.outcome])].join(' ').toLowerCase();
    return terms.every((t) => text.includes(t));
  });
}

export function useEventList(): { rows: ComputedRef<EventRecord[]>; count: ComputedRef<number>; hidden: ComputedRef<number>; query: WritableComputedRef<string>; more: () => void } {
  const query = useQueryRef('q', '');
  const shown = ref(PAGE);
  const all = computed(() => narrowedEvents(catalog.events, query.value));
  return {
    rows: computed(() => all.value.slice(0, shown.value)),
    count: computed(() => all.value.length),
    hidden: computed(() => Math.max(0, all.value.length - shown.value)),
    query,
    more: () => (shown.value += PAGE),
  };
}
