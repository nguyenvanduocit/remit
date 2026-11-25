// The replica of the event open on screen: what the server answered (what arrived and the firings it caused) and the pushes
// that change it afterwards. The core keeps it (replica.ts): the Events page opens it (EventsView) and closes it, a
// reconnect opens it again. Drawn here as the core hands its model; nothing here is decided in the browser.
import type { EventRecord } from './core/generated/types.generated';
import { oneOf } from './core/client';
import { ask, coreState, useModel } from './replica';

const open = useModel<EventRecord>({ model: 'openEvent' });

export interface OpenEvent {
  readonly event: EventRecord | null;
  readonly opening: string; // the event being opened, until it shows
}

export const openedEvent: OpenEvent = {
  get event() {
    return oneOf(open);
  },
  get opening() {
    return coreState.value?.eventOpening ?? '';
  },
};

// Opens the event: the last one asked for is the one that shows.
export function openEvent(id: string): void {
  ask({ ask: 'openEvent', id });
}

// Its page is gone: nothing of the event stays held, and an opening still on its way is dropped.
export function closeEvent(): void {
  ask({ ask: 'closeEvent' });
}
