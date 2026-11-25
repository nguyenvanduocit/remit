// The newest part of a long list, and a page more each time the owner asks for older: a chain of thousands of events draws
// only what is read (the Live feed, an agent's activity).
import { computed, ref } from 'vue';

const PAGE = 120;

export function useNewest<T>(all: () => readonly T[]) {
  const limit = ref(PAGE);
  const shown = computed(() => all().slice(-limit.value));
  const older = computed(() => all().length - shown.value.length);
  return { shown, older, more: () => (limit.value += PAGE) };
}
