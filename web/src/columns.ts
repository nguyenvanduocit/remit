// A board shown one column at a time beside the item that is open (the board's columns beside a chain, the Signals columns
// beside a subscription): the column shown is the one that holds the open item, so it follows the item as it moves, unless the
// owner slid to another (?col=). Sliding back to the item's own clears it. Which way the next one slides in is the way it
// lies: a later column from the right, an earlier one from the left.
import { computed, ref, watch, type ComputedRef, type Ref } from 'vue';
import { useQueryRef } from './nav';

export function useColumnSlide<K extends string>(keys: () => readonly K[], own: () => K | undefined): { shownKey: ComputedRef<K>; at: ComputedRef<number>; go: (key: K) => void; slide: Ref<'next' | 'prev'> } {
  const picked = useQueryRef<string>('col', '');
  const shownKey = computed<K>(() => keys().find((k) => k === picked.value) ?? own() ?? keys()[0]);
  const at = computed(() => Math.max(0, keys().indexOf(shownKey.value)));
  const go = (key: K) => (picked.value = key === own() ? '' : key);
  const slide = ref<'next' | 'prev'>('next');
  watch(at, (now, was) => (slide.value = now > was ? 'next' : 'prev'));
  return { shownKey, at, go, slide };
}
