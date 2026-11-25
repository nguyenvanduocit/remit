// Browser notifications for what newly needs the owner (ADR-0078): only a wait that arrives in the board's list unseen while
// the page is open, and only while he is not looking at the page (visible and focused, he sees the board's count rise). One
// notification per push, however many arrived in it, tagged so two open tabs show one. Nothing else notifies. The browser asks
// his permission only when he asks for notifications (the palette's row, a gesture of his), never on load. With the sound on
// (sound.ts), the same arrival plays it, whether or not he is looking: he turned it on to hear it.
import { watch } from 'vue';
import { catalog } from './catalog';
import type { Wait } from './core/generated/types.generated';
import { chime, soundOn } from './sound';
import { plural } from './ui';
import { useOpenWait } from './waits';

const TAG = 'remit-waiting';
// How many of the waits that arrived together the notification names; the rest it counts.
const NAMED = 3;

export const notificationsSupported = (): boolean => typeof Notification !== 'undefined';
// The browser has not been asked yet: the owner can turn notifications on.
export const canAskToNotify = (): boolean => notificationsSupported() && Notification.permission === 'default';
// Asks the browser for permission; called from the owner's gesture only.
export const askToNotify = async (): Promise<NotificationPermission> => (notificationsSupported() ? Notification.requestPermission() : 'denied');

// What a notification says of the waits that arrived together: the board's count, then each wait's kind and ask.
export function notificationOf(fresh: readonly Wait[], needsYou: number): { title: string; body: string } {
  const named = fresh.slice(0, NAMED).map((w) => `${w.label}: ${w.ask}`);
  if (fresh.length > NAMED) named.push(`and ${plural(fresh.length - NAMED, 'more')}`);
  return { title: `Remit · ${needsYou} need you`, body: named.join('\n') };
}

// Watches the board's list for the life of the page; a click on a notification opens the first wait it names.
export function useWaitNotifications(): void {
  const open = useOpenWait();
  // The waits the page has already been shown, by id; none until the first board, which notifies of nothing.
  let known: Set<string> | undefined;
  watch(
    () => catalog.board,
    (board) => {
      if (!board) return;
      const was = known;
      known = new Set(board.waiting.map((w) => w.id));
      if (!was) return;
      const fresh = board.waiting.filter((w) => w.unseen && !was.has(w.id));
      if (fresh.length && soundOn.value) chime();
      const looking = document.visibilityState === 'visible' && document.hasFocus();
      if (!fresh.length || looking || !notificationsSupported() || Notification.permission !== 'granted') return;
      const { title, body } = notificationOf(fresh, board.needsYou);
      const n = new Notification(title, { body, tag: TAG });
      n.onclick = () => {
        window.focus();
        open(fresh[0]);
        n.close();
      };
    },
    { immediate: true },
  );
}
