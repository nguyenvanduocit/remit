// The replica of the chain open on screen: its view (chain, cards, edges, decisions, landings, the owner's drafts, the
// mentors' helps, its workflow runs, where each agent stands, how its result has been judged, what happened while the owner was
// away, the coach's retrospectives, its landings with their releases, where Datadog shows it) and its events. The core
// keeps it (replica.ts): the route asks it to open a chain (ChainsView), it opens it again after a reconnect and closes it
// when the server deletes it, and applies what is pushed for it meanwhile. Drawn here as the core hands its models; a
// card's own stream is drawn by what shows it (useCardEvents). Nothing here is decided in the browser.
import { computed, effectScope, shallowRef, toValue, watch, type ComputedRef, type MaybeRefOrGetter } from 'vue';
import type { AgentStanding, Away, Card, ChainRead, ChainView, DatadogLink, DecisionRead, Draft, Edge, EventRead, Help, InboxItem, LandingRead, RetrospectiveRead, Verification, Workflow } from './core/generated/types.generated';
import type { Freshness } from './core/generated/replica/types.generated';
import { inOrder, oneOf, type Shown } from './core/client';
import { catalog, standingOf } from './catalog';
import { ask, coreState, useModel } from './replica';

export interface OpenChain {
  readonly view: ChainView | null;
  readonly opening: string; // the chain being opened, until it shows
  readonly events: readonly EventRead[]; // the chain's newest events, oldest first: as answered, then as pushed, as many as the core keeps
  readonly status: Freshness; // how current its events are
  readonly truncated: boolean; // the answer held the newest events only: the oldest are missing
}

const chain = useModel<ChainRead>({ model: 'openChain' });
const verification = useModel<Verification>({ model: 'verification' });
const away = useModel<Away>({ model: 'away' });
const cardRows = useModel<Card>({ model: 'cards' });
const cards = inOrder(cardRows);
const edges = inOrder(useModel<Edge>({ model: 'edges' }));
const decisions = inOrder(useModel<DecisionRead>({ model: 'decisions' }));
const landings = inOrder(useModel<LandingRead>({ model: 'landings' }));
const datadog = inOrder(useModel<DatadogLink>({ model: 'datadogLinks' }));
const drafts = inOrder(useModel<Draft>({ model: 'chainDrafts' }));
const workflows = inOrder(useModel<Workflow>({ model: 'workflows' }));
const helps = inOrder(useModel<Help>({ model: 'helps' }));
const agentRows = useModel<AgentStanding>({ model: 'agentStandings' });
const agents = inOrder(agentRows);
const retrospectives = inOrder(useModel<RetrospectiveRead>({ model: 'retrospectives' }));
const eventRows = useModel<EventRead>({ model: 'chainEvents' });
const events = inOrder(eventRows);

const view = computed((): ChainView | null => {
  const c = oneOf(chain);
  const v = oneOf(verification);
  if (!c || !v) return null;
  const a = oneOf(away);
  return {
    chain: c,
    cards: cards.value,
    edges: edges.value,
    decisions: decisions.value,
    landings: landings.value,
    datadog: datadog.value,
    drafts: drafts.value,
    workflows: workflows.value,
    helps: helps.value,
    agents: agents.value,
    verification: v,
    standing: standingOf(c.chain.id),
    ...(a && { away: a }),
    retrospectives: retrospectives.value,
  };
});

export const opened: OpenChain = {
  get view() {
    return view.value;
  },
  get opening() {
    return coreState.value?.opening ?? '';
  },
  get events() {
    return events.value;
  },
  get status() {
    return eventRows.status.value;
  },
  get truncated() {
    return coreState.value?.truncated ?? false;
  },
};

// Opens the chain: the last one asked for is the one that shows, and the one a reconnect opens again.
export function openChain(id: string): void {
  ask({ ask: 'openChain', id });
}

// A card's whole stream, for the drawer, when the chain's answer did not hold all of it.
export function loadStream(cardId: string): void {
  ask({ ask: 'loadStream', card: cardId });
}

// Whether the card's stream goes back further than the events held.
export const hasEarlier = (cardId: string): boolean => coreState.value?.earlier.includes(cardId) ?? false;

// The card's page of events before the oldest held, as the owner asks for more of its past.
export function loadEarlier(cardId: string): void {
  ask({ ask: 'loadEarlier', card: cardId });
}

// The helps of card cardId the server holds, oldest offer first.
export const helpsOf = (cardId: string): Help[] => helps.value.filter((h) => h.card === cardId);
// The card of the open chain with id, when it holds one.
export const cardOf = (id: string): Card | undefined => cardRows.rows.get(id);

// The events known for a card: its whole stream when the chain's answer held it or it was asked for, else what there is.
// Drawn while the caller's scope lives, for the card `cardId` names now: a drawer that closes or moves to another card
// hides the stream it drew. One list until a row changes, so a drawer handed it is not handed a new one at each render.
export function useCardEvents(cardId: MaybeRefOrGetter<string>): ComputedRef<readonly EventRead[]> {
  const shown = shallowRef<Shown<EventRead>>();
  watch(
    () => toValue(cardId),
    (card, _, onCleanup) => {
      const scope = effectScope(true);
      shown.value = scope.run(() => useModel<EventRead>({ model: 'cardEvents', card }));
      onCleanup(() => scope.stop());
    },
    { immediate: true },
  );
  return computed(() => (shown.value ? [...shown.value.rows.values()] : []));
}

// The Chair's card of a chain.
export const chairOf = (v: ChainView): Card | undefined => v.cards.find((c) => c.id === v.chain.chain.chairId);

// Where an agent of the open chain stands, as the server says: its badge, what it is doing, what waits on the owner with
// it. A card the server has not stood yet (just hired, before its push) stands as nothing at all.
export const NOWHERE: AgentStanding = { cardId: '', home: '', label: '', tone: 'idle', doing: '', doingTone: 'idle', work: '', warms: false, waiting: [], children: [], with: [], peers: [], branch: { agents: 0, turns: 0, working: false }, asks: [], stopSays: '', remembers: false, rev: 0 };
export const agentOf = (cardId: string): AgentStanding => agentRows.rows.get(cardId) ?? NOWHERE;

const NO_ITEMS: readonly InboxItem[] = [];
// What waits on the owner with an agent of the open chain (the server's AgentStanding.waiting), as the inbox holds it:
// its drawer shows it, and every view the agent is in marks it.
export function waitingOn(cardId: string): readonly InboxItem[] {
  const ids = agentOf(cardId).waiting;
  return ids.length ? catalog.inbox.filter((i) => ids.includes(i.id)) : NO_ITEMS;
}
