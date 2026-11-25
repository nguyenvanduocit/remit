// What agents told the organisation as a board, on the Feedback page and beside one open: the server's columns, one per kind,
// each holding its feedback newest first (catalog.feedback, the server's newest 200), narrowed by what the owner types
// (?q=), a page at a time.
import { computed, reactive, type ComputedRef, type WritableComputedRef } from 'vue';
import { catalog } from './catalog';
import { useQueryRef } from './nav';
import type { Feedback, FeedbackColumn, FeedbackKind } from './core/generated/types.generated';

// The server's columns, one per kind of feedback, in the order they stand (Boards.feedback).
const columnsOf = (): readonly FeedbackColumn[] => catalog.boards?.feedback ?? [];
export const feedbackKindKeys = (): FeedbackKind[] => columnsOf().map((c) => c.key);
// A kind's column as the server names it; none before the first snapshot.
export const feedbackKindOf = (kind: FeedbackKind): FeedbackColumn | undefined => columnsOf().find((c) => c.key === kind);

// A column as it shows: the server's column, its feedback (a page of them), count, how many it holds, and hidden, how many are not
// shown.
export type ShownFeedbackColumn = FeedbackColumn & { rows: Feedback[]; count: number; hidden: number };

const PAGE = 30;

// What the search narrows the feedback to: every word has to appear in who sent it, what it says, its agent or its model.
export function narrowedFeedback(rows: readonly Feedback[], query: string): Feedback[] {
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (!terms.length) return [...rows];
  return rows.filter((f) => terms.every((t) => [f.name, f.text, f.agent, f.model, f.effort ?? ''].join(' ').toLowerCase().includes(t)));
}

// The columns, each with the first `shown[kind]` (or a page) of its feedback in the server's order.
export function feedbackColumnsOf(columns: readonly FeedbackColumn[], rows: readonly Feedback[], shown: Partial<Record<FeedbackKind, number>>): ShownFeedbackColumn[] {
  return columns.map((col) => {
    const all = rows.filter((f) => f.kind === col.key);
    const page = all.slice(0, shown[col.key] ?? PAGE);
    return { ...col, rows: page, count: all.length, hidden: all.length - page.length };
  });
}

export function useFeedbackColumns(): { columns: ComputedRef<ShownFeedbackColumn[]>; query: WritableComputedRef<string>; more: (kind: FeedbackKind) => void } {
  const query = useQueryRef('q', '');
  const shown = reactive<Partial<Record<FeedbackKind, number>>>({});
  const columns = computed(() => feedbackColumnsOf(columnsOf(), narrowedFeedback(catalog.feedback, query.value), shown));
  const more = (kind: FeedbackKind) => (shown[kind] = (shown[kind] ?? PAGE) + PAGE);
  return { columns, query, more };
}
