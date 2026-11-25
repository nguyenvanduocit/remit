// The rows of the Signals table, read from the server (signalRows): every reading and every firing whose key matches what the
// owner types, newest first, a page at a time. What is typed is a key pattern (a GLOB: `fact:*`, `chain:*.live`); words with no
// `*` or `?` find the keys that hold them. The rows are read again when the board moves (a reading came or something fired) and
// when the link is live again. Holds what the server answered; decides nothing.
import { computed, ref, shallowRef, watch, type Ref } from 'vue';
import { useApi } from './core/api';
import { session } from './session';
import type { SignalRow } from './core/generated/types.generated';

const PAGE = 100;

// What is typed as the pattern the server matches keys with.
export const globOf = (query: string): string => {
  const t = query.trim();
  if (!t) return '*';
  return /[*?[]/.test(t) ? t : `*${t}*`;
};

export function useSignalRows(query: Ref<string>, version: Ref<string>) {
  const api = useApi();
  const rows = shallowRef<SignalRow[]>([]);
  const status = ref<'loading' | 'ready' | 'failed'>('loading');
  const limit = ref(PAGE);
  const pattern = computed(() => globOf(query.value));
  // Reading one pattern after another ends on the last one asked for: an earlier answer arriving late is dropped.
  let seq = 0;
  async function read() {
    const mine = ++seq;
    try {
      const answer = await api.signalRows(pattern.value, limit.value);
      if (mine !== seq) return;
      rows.value = answer;
      status.value = 'ready';
    } catch {
      if (mine === seq) status.value = 'failed';
    }
  }
  watch(pattern, () => {
    limit.value = PAGE;
    status.value = 'loading';
  });
  watch([pattern, limit, version, () => session.link.phase === 'live'], () => void read(), { immediate: true });
  return {
    rows,
    status,
    // A full page says there may be more.
    full: computed(() => rows.value.length >= limit.value),
    more: () => (limit.value += PAGE),
  };
}
