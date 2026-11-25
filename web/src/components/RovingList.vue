<script setup lang="ts" generic="T">
import { computed } from 'vue';

// A list of cards the keyboard crosses as one stop (a roving tabindex, WAI-ARIA's composite pattern): Tab lands on one card,
// the one last used (`current`, else the first), and goes on through that card's own controls and out of the list; Up and
// Down move to the card before or after it, Home and End to the first and last. The slot renders each item as an `li` with
// `data-roving="<id>"`, the card and its controls taking Tab only while `stop` is theirs, so a long book is a few stops,
// not two for every card. Focus coming into a card by the pointer makes it the current one. The caller names the list and
// says how it is crossed (aria-label, aria-describedby).
const props = defineProps<{ items: readonly T[]; idOf: (item: T) => string }>();
const current = defineModel<string>('current', { default: '' });
const stop = computed(() => {
  const ids = props.items.map(props.idOf);
  return ids.includes(current.value) ? current.value : (ids[0] ?? '');
});

const cardOf = (e: Event, list: HTMLElement) => {
  const card = e.target instanceof Element ? e.target.closest<HTMLElement>('[data-roving]') : null;
  return card?.parentElement === list ? card : null;
};
function onFocusIn(e: FocusEvent) {
  const card = cardOf(e, e.currentTarget as HTMLElement);
  if (card?.dataset.roving) current.value = card.dataset.roving;
}
const STEP: Record<string, (at: number, last: number) => number> = {
  ArrowDown: (at, last) => Math.min(last, at + 1),
  ArrowUp: (at) => Math.max(0, at - 1),
  Home: () => 0,
  End: (_, last) => last,
};
function onKey(e: KeyboardEvent) {
  const step = STEP[e.key];
  const list = e.currentTarget as HTMLElement;
  const card = cardOf(e, list);
  if (!step || !card || e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
  e.preventDefault();
  const cards = [...list.querySelectorAll<HTMLElement>(':scope > [data-roving]')];
  const next = cards[step(cards.indexOf(card), cards.length - 1)];
  if (next?.dataset.roving) current.value = next.dataset.roving;
  next?.focus();
}
</script>

<template>
  <ul class="m-0 flex list-none flex-col gap-2 p-0" @focusin="onFocusIn" @keydown="onKey">
    <template v-for="item in items" :key="idOf(item)"><slot :item="item" :stop="idOf(item) === stop" /></template>
  </ul>
</template>
