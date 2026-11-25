// The Subscriptions board as the owner sees it, on the Subscriptions page and beside an open subscription: every subscription in the
// server's columns (Boards.subscriptions, by SubscriptionLine.group) and order, narrowed by what the owner types (?q=) and what he
// sets each kind to (?kind=, ?hide=), each column a page at a time.
import { computed, reactive, type ComputedRef, type WritableComputedRef } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { catalog } from './catalog';
import { useQueryRef } from './nav';
import type { SubscriptionColumn, SubscriptionGroup, SubscriptionKind, SubscriptionKindSaid, SubscriptionLine } from './core/generated/types.generated';

// A column as it shows: the server's column, its lines (a page of them), count, how many it holds, and hidden, how many of them
// are not shown.
export type ShownSubscriptionColumn = SubscriptionColumn & { lines: SubscriptionLine[]; count: number; hidden: number };

const PAGE = 30;

// The server's columns in its order, and its kinds of subscription with their words, icons and whether each starts hidden.
const columnsOf = (): readonly SubscriptionColumn[] => catalog.boards?.subscriptions ?? [];
export const subscriptionKinds = (): readonly SubscriptionKindSaid[] => catalog.boards?.subscriptionKinds ?? [];
export const subscriptionColumnKeys = (): SubscriptionGroup[] => columnsOf().map((c) => c.key);
// A kind's words and icon as the server says them; none before the first snapshot.
export const kindSaidOf = (kind: SubscriptionKind): SubscriptionKindSaid | undefined => subscriptionKinds().find((k) => k.kind === kind);

// Each kind is any (it shows unless some kind is marked only), only (the kinds marked only are all that show) or hide (it does
// not show). A click on a kind moves it to the next state. A kind the states do not name is as it starts.
export type KindState = 'any' | 'only' | 'hide';
export type KindStates = Partial<Record<SubscriptionKind, KindState>>;
const NEXT: Record<KindState, KindState> = { any: 'only', only: 'hide', hide: 'any' };
export const nextKindState = (s: KindState): KindState => NEXT[s];
export const kindStateOf = (states: KindStates, kind: SubscriptionKind): KindState => states[kind] ?? 'any';

const kindsIn = (kinds: readonly SubscriptionKindSaid[], states: KindStates, state: KindState) => kinds.filter((k) => kindStateOf(states, k.kind) === state);
const startsHidden = (kinds: readonly SubscriptionKindSaid[]) => kinds.filter((k) => k.startsHidden).map((k) => k.kind);
const listOf = (kinds: readonly SubscriptionKindSaid[], v: string): SubscriptionKind[] => v.split(',').flatMap((name) => kinds.filter((k) => k.kind === name).map((k) => k.kind));

// The states from the address: ?kind= lists the kinds marked only, ?hide= the hidden ones (absent: the ones the server starts
// hidden); a kind in both is only.
export function kindStatesOf(kinds: readonly SubscriptionKindSaid[], kind: string | undefined, hide: string | undefined): KindStates {
  const only = listOf(kinds, kind ?? '');
  const hidden = hide === undefined ? startsHidden(kinds) : listOf(kinds, hide);
  const states: KindStates = {};
  for (const k of kinds) states[k.kind] = only.includes(k.kind) ? 'only' : hidden.includes(k.kind) ? 'hide' : 'any';
  return states;
}

// The address of the states, the reverse of kindStatesOf: nothing for what is as it starts.
export function kindQueryOf(kinds: readonly SubscriptionKindSaid[], states: KindStates): { kind: string | undefined; hide: string | undefined } {
  const only = kindsIn(kinds, states, 'only').map((k) => k.kind).join(',');
  const hide = kindsIn(kinds, states, 'hide').map((k) => k.kind).join(',');
  return { kind: only || undefined, hide: hide === startsHidden(kinds).join(',') ? undefined : hide };
}

// What the states say in a few words: the kinds marked only, or all but the hidden ones.
export function kindSummary(kinds: readonly SubscriptionKindSaid[], states: KindStates): string {
  const only = kindsIn(kinds, states, 'only');
  if (only.length) return only.map((k) => k.plural).join(', ');
  const hidden = kindsIn(kinds, states, 'hide');
  return hidden.length ? `All but ${hidden.map((k) => k.plural).join(', ')}` : 'All kinds';
}

// What the search and the kinds narrow the lines to: every word has to appear in a line's title, what its kind does, its signal
// or what it does, and the line's kind has to show (kindSummary). A held line shows whatever its kind: it waits for the owner,
// and the tab counts it (SignalBoard.held), so hiding it would leave a count with nothing behind it.
export function narrowedLines(lines: readonly SubscriptionLine[], query: string, states: KindStates): SubscriptionLine[] {
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
  const has = (l: SubscriptionLine) => !terms.length || terms.every((t) => [l.title, kindSaidOf(l.kind)?.says ?? '', l.key ?? '', l.what].join(' ').toLowerCase().includes(t));
  const anyOnly = Object.values(states).includes('only');
  const shows = (l: SubscriptionLine) => !!l.hold || (anyOnly ? kindStateOf(states, l.kind) === 'only' : kindStateOf(states, l.kind) !== 'hide');
  return lines.filter((l) => shows(l) && has(l));
}

// The server's columns, each with the first `shown[key]` (or a page) of its lines in the server's order.
export function subscriptionColumnsOf(columns: readonly SubscriptionColumn[], lines: readonly SubscriptionLine[], shown: Partial<Record<SubscriptionGroup, number>>): ShownSubscriptionColumn[] {
  return columns.map((col) => {
    const all = lines.filter((l) => l.group === col.key);
    const page = all.slice(0, shown[col.key] ?? PAGE);
    return { ...col, lines: page, count: all.length, hidden: all.length - page.length };
  });
}

export function useSubscriptionColumns(): {
  columns: ComputedRef<ShownSubscriptionColumn[]>;
  query: WritableComputedRef<string>;
  kinds: WritableComputedRef<KindStates>;
  kindCounts: ComputedRef<Partial<Record<SubscriptionKind, number>>>;
  more: (key: SubscriptionGroup) => void;
} {
  const route = useRoute();
  const router = useRouter();
  const query = useQueryRef('q', '');
  const param = (key: string) => (typeof route.query[key] === 'string' ? route.query[key] : undefined);
  // Both params change in one step, so the second does not write over the first.
  const kinds = computed<KindStates>({
    get: () => kindStatesOf(subscriptionKinds(), param('kind'), param('hide')),
    set: (v) => void router.replace({ query: { ...route.query, ...kindQueryOf(subscriptionKinds(), v) } }),
  });
  const shown = reactive<Partial<Record<SubscriptionGroup, number>>>({});
  const lines = computed(() => catalog.signals?.subscriptions ?? []);
  const columns = computed(() => subscriptionColumnsOf(columnsOf(), narrowedLines(lines.value, query.value, kinds.value), shown));
  const kindCounts = computed(() => {
    const n: Partial<Record<SubscriptionKind, number>> = {};
    for (const l of lines.value) n[l.kind] = (n[l.kind] ?? 0) + 1;
    return n;
  });
  const more = (key: SubscriptionGroup) => (shown[key] = (shown[key] ?? PAGE) + PAGE);
  return { columns, query, kinds, kindCounts, more };
}
