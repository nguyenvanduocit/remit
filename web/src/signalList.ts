// What Remit can hear, as a list on the Signals page and beside an open signal: every signal the board lists in the server's
// order (catalog.signals.signals, the newest reading first), narrowed by what the owner types (?q=), a page at a time. A signal is a
// row and its key is the index: a word narrows to the keys or titles that hold it, a pattern with `*` (`fact:*`, `chain:*.live`) to
// the keys it matches whole. The key shapes the server knows (catalog.signals.sources) are offered as patterns; typed exactly, the
// one typed says what its signals are.
import { computed, ref, type ComputedRef, type WritableComputedRef } from 'vue';
import { catalog } from './catalog';
import { useQueryRef } from './nav';
import { matchesKey, patternOfShape } from './pattern';
import type { SignalLine, SignalSourceLine } from './core/generated/types.generated';

const PAGE = 30;

// A key shape of the server as a pattern the owner types: each <part> any run of characters, a run of them one `*`
// (`fact:<noun>.<verb>` is `fact:*`, `chain:<chain>.landing.<repository>` is `chain:*.landing.*`).
export const patternOf = (s: SignalSourceLine): string => patternOfShape(s.pattern);

// Whether a signal answers one typed term: a pattern its whole key matches (the server's GLOB, pattern.ts), or a word in its key or title.
function matches(s: SignalLine, term: string): boolean {
  if (!/[*?[]/.test(term)) return `${s.key} ${s.title}`.toLowerCase().includes(term);
  return matchesKey(term, s.key.toLowerCase());
}

// The signals that answer every term typed.
export function narrowedSignals(signals: readonly SignalLine[], query: string): SignalLine[] {
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
  return signals.filter((s) => terms.every((t) => matches(s, t)));
}

export function useSignalList(): {
  rows: ComputedRef<SignalLine[]>;
  count: ComputedRef<number>;
  hidden: ComputedRef<number>;
  unlisted: ComputedRef<number>;
  query: WritableComputedRef<string>;
  picked: ComputedRef<SignalSourceLine | undefined>;
  sources: ComputedRef<SignalSourceLine[]>;
  total: ComputedRef<number>;
  more: () => void;
} {
  const query = useQueryRef('q', '');
  const shown = ref(PAGE);
  const sources = computed(() => catalog.signals?.sources ?? []);
  // The key shape typed exactly, whose words the list opens with.
  const picked = computed(() => sources.value.find((s) => patternOf(s) === query.value.trim()));
  const all = computed(() => narrowedSignals(catalog.signals?.signals ?? [], query.value));
  // The board lists the newest signals only: how many of the key shape typed (or of all, while nothing is typed) it left out.
  const unlisted = computed(() => {
    if (picked.value) return Math.max(picked.value.signals - all.value.length, 0);
    return query.value.trim() ? 0 : (catalog.signals?.signalsMore ?? 0);
  });
  return {
    rows: computed(() => all.value.slice(0, shown.value)),
    count: computed(() => all.value.length),
    hidden: computed(() => Math.max(0, all.value.length - shown.value)),
    unlisted,
    query,
    picked,
    sources,
    total: computed(() => (catalog.signals?.signals.length ?? 0) + (catalog.signals?.signalsMore ?? 0)),
    more: () => (shown.value += PAGE),
  };
}
