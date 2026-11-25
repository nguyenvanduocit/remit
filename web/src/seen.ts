// The owner viewed something (ADR-0079): the browser tells the server only for a real view, never on navigation and never from
// a hidden tab. A view is the page visible and focused while the newest line of what is shown (a chain's newest run, the
// Assistant feed's newest line) has been in the viewport for a second. The server keeps the mark and moves it only forward,
// so a call made twice, or late, changes nothing.
import { onBeforeUnmount, watch, type Ref } from 'vue';
import { useEventListener, useIntersectionObserver } from '@vueuse/core';
import { useApi } from './core/api';
import type { SeenSubject } from './core/generated/types.generated';

// How long the newest line has to stay in sight before it counts as seen.
export const SEEN_AFTER_MS = 1000;

const keyOf = (s: SeenSubject): string => (s.kind === 'home' ? `home:${s.home}:${s.eventId}` : 'assistant_feed');

// Marks `subject` seen once `newest` (the element of the newest line, or under it) has been in sight for SEEN_AFTER_MS on a visible,
// focused page. `subject` is undefined while there is nothing new to mark; a subject already marked is not marked again until
// it has been.
export function useSeenMark(subject: () => SeenSubject | undefined, newest: Ref<HTMLElement | null>): void {
  const api = useApi();
  let inSight = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let marked = '';
  const viewing = () => inSight && document.visibilityState === 'visible' && document.hasFocus();
  // Marks what is newest once the view has lasted SEEN_AFTER_MS. New lines arriving meanwhile do not restart the wait: a
  // chain streaming faster than once a second is still marked, at most once a second, while the owner watches it.
  const mark = () => {
    timer = undefined;
    const s = subject();
    if (!s || keyOf(s) === marked || !viewing()) return;
    marked = keyOf(s);
    // A mark that did not reach the server is made again at the next view; nothing to tell the owner.
    api.seenMark(s).catch(() => (marked = ''));
  };
  const settle = () => {
    const s = subject();
    // Nothing new to mark: the next subject is marked even when it names the same thing (the feed, once new lines came).
    if (!s) marked = '';
    if (!viewing()) {
      clearTimeout(timer);
      timer = undefined;
    } else if (s && keyOf(s) !== marked && !timer) timer = setTimeout(mark, SEEN_AFTER_MS);
  };
  useIntersectionObserver(newest, ([entry]) => {
    inSight = !!entry?.isIntersecting;
    settle();
  });
  useEventListener(document, 'visibilitychange', settle);
  useEventListener(window, 'focus', settle);
  useEventListener(window, 'blur', settle);
  watch(() => {
    const s = subject();
    return s && keyOf(s);
  }, settle);
  onBeforeUnmount(() => clearTimeout(timer));
}
