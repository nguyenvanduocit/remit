// A feed whose scroll position is the owner's: it follows what is being written while he is at its newest line, and lets go
// only when he moves up himself (the wheel, a drag of the scrollbar or a finger, a key that scrolls up), never because the
// feed moved under him (a row folding at the end, the page's own scroll). Let go, it stays where he reads and counts what
// arrived since (`unseen`: the events newer than the newest when he let go, by id, so a window that drops its oldest
// or a page of older events read back does not move it); back at the bottom (within SLACK px) it follows again. `follow` goes to
// the bottom and sticks (the jump button, another agent picked, a message sent).
import { useEventListener } from '@vueuse/core';
import { nextTick, ref, watch, type Ref, type WatchSource } from 'vue';
import type { EventRead } from './core/generated/types.generated';

const SLACK = 60;
// A scroll counts as the owner's within this long of his gesture.
const GESTURE_MS = 1500;
const UP_KEYS = new Set(['ArrowUp', 'PageUp', 'Home']);

export function useStickToBottom(el: Ref<HTMLElement | null | undefined>, grows: WatchSource, events: () => readonly EventRead[]) {
  const newest = () => events().at(-1)?.event.id ?? 0;
  const stuck = ref(true);
  const seen = ref(newest());
  const unseen = ref(0);
  let gestureAt = 0;
  const gesture = () => (gestureAt = Date.now());
  const toBottom = () =>
    nextTick(() => {
      if (el.value) el.value.scrollTop = el.value.scrollHeight;
    });
  const stick = () => {
    stuck.value = true;
    seen.value = newest();
    unseen.value = 0;
  };
  watch(grows, () => {
    if (stuck.value) {
      seen.value = newest();
      void toBottom();
    } else unseen.value = newerThan(events(), seen.value);
  }, { immediate: true });

  useEventListener(el, 'wheel', (e: WheelEvent) => e.deltaY < 0 && gesture(), { passive: true });
  useEventListener(el, 'pointerdown', gesture, { passive: true }); // a drag of the scrollbar, a finger on a touch screen
  useEventListener(el, 'keydown', (e: KeyboardEvent) => (UP_KEYS.has(e.key) || (e.key === ' ' && e.shiftKey)) && gesture());

  const onScroll = () => {
    const e = el.value;
    if (!e) return;
    if (e.scrollHeight - e.scrollTop - e.clientHeight < SLACK) stick();
    else if (stuck.value && Date.now() - gestureAt < GESTURE_MS) stuck.value = false;
  };
  const follow = () => {
    stick();
    return toBottom();
  };
  return { stuck, unseen, onScroll, follow };
}

// How many of events (oldest first) have an id above id: read from the end, where the new ones are.
function newerThan(events: readonly EventRead[], id: number): number {
  let n = 0;
  for (let i = events.length - 1; i >= 0 && events[i].event.id > id; i--) n++;
  return n;
}
