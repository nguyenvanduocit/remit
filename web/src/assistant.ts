// The replica of the Assistant, the owner's agent for the whole app: its card, where it stands, the owner's unsent words to
// it, its stream, which is the conversation, and its feed: what it decided for the owner (ADR-0062), which the server
// sends whole each time it moves. The core keeps it (replica.ts): the snapshot brings the view and the feed, pushes for
// its home change them, and the stream is asked for when the owner first opens the Assistant (loadAssistant), and again
// after a reconnect once it has been. Drawn here as the core hands its models; nothing here is decided in the browser.
import { computed } from 'vue';
import type { AgentStanding, Assistant, AssistantFeed, AssistantView, Card, Draft, EventRead, InboxItem } from './core/generated/types.generated';
import type { Freshness } from './core/generated/replica/types.generated';
import { inOrder, oneOf } from './core/client';
import { catalog } from './catalog';
import { ask, coreState, useModel } from './replica';

export interface AssistantReplica {
  readonly view: AssistantView | null; // none until the first snapshot
  readonly events: readonly EventRead[]; // its card's stream, oldest first: as answered, then as pushed
  readonly status: Freshness; // how current its stream is
  readonly loaded: boolean; // the stream has been answered
  readonly feed: AssistantFeed | null; // none until the first snapshot
}

const me = useModel<Assistant>({ model: 'assistant' });
const card = useModel<Card>({ model: 'assistantCard' });
const agent = useModel<AgentStanding>({ model: 'assistantAgent' });
const feed = useModel<AssistantFeed>({ model: 'assistantFeed' });
const drafts = inOrder(useModel<Draft>({ model: 'assistantDrafts' }));
const eventRows = useModel<EventRead>({ model: 'assistantEvents' });
const events = inOrder(eventRows);

const view = computed((): AssistantView | null => {
  const a = oneOf(me);
  const c = oneOf(card);
  const s = oneOf(agent);
  return a && c && s ? { assistant: a, card: c, agent: s, drafts: drafts.value } : null;
});

export const assistant: AssistantReplica = {
  get view() {
    return view.value;
  },
  get events() {
    return events.value;
  },
  get status() {
    return eventRows.status.value;
  },
  get loaded() {
    return coreState.value?.assistantLoaded ?? false;
  },
  get feed() {
    return oneOf(feed);
  },
};

// Asks for the Assistant's stream; a reconnect asks again once it has been asked for.
export function loadAssistant(): void {
  ask({ ask: 'loadAssistant' });
}

const NO_ITEMS: readonly InboxItem[] = [];
// What waits on the owner with the Assistant (its standing's `waiting`), as the inbox holds it.
export const assistantWaiting = computed((): readonly InboxItem[] => {
  const ids = view.value?.agent.waiting ?? [];
  return ids.length ? catalog.inbox.filter((i) => ids.includes(i.id)) : NO_ITEMS;
});
