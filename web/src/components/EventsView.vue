<script setup lang="ts">
import { computed, onBeforeUnmount, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { catalog, EVENTS_KEPT } from '../catalog';
import { useEventList } from '../eventList';
import { onScreen } from '../nav';
import { closeEvent, openEvent, openedEvent } from '../openEvent';
import { pathOf } from '../eventSignals';
import EventsSidebar from './EventsSidebar.vue';
import EventList from './EventList.vue';
import EventPage from './EventPage.vue';
import EmptyPage from './EmptyPage.vue';
import EmptyState from './EmptyState.vue';
import ListPage from './ListPage.vue';
import PageBody from './PageBody.vue';
import PageTabs from './PageTabs.vue';
import SearchBox from './SearchBox.vue';

// Everything that came in, whichever way (a post to a signal's door, `remit emit`, a sensor, an agent), with what became of it
// (catalog.events, the server's newest 200): one list, newest first, a page at a time. With nothing open, the page is the
// search and the list. A card opens its event: the address names it (`event:<signal>#<rev>`), the list folds beside it
// (EventsSidebar), and the page is the event's (the firings it caused, what arrived); it is read when the page opens and after a
// reconnect, and live while it is open (openEvent.ts). With nothing come in yet, the page says what an event is and how one comes
// (EmptyPage). The event signals themselves are on the Signals tab, and what is done about an event is a subscription, on the
// Subscriptions tab.
const route = useRoute();
const router = useRouter();
const { rows, count, hidden, query, more } = useEventList();
// Where any event signal's door is, for the page that has none yet.
const ANY_PATH = pathOf('<name>');
const id = computed(() => (typeof route.params.id === 'string' ? route.params.id : ''));

watch(
  id,
  (now) => {
    if (now) openEvent(now);
    else closeEvent();
  },
  { immediate: true },
);
onBeforeUnmount(closeEvent);
// What is on screen of the event the address names; while another is on its way, the old one steps back (ListPage busy).
const screen = computed(() => onScreen(openedEvent.event?.id, id.value, openedEvent.opening));
const event = computed(() => (screen.value.shows ? openedEvent.event : null));

// Esc on the open event's page goes back to the whole list; in a search with words in it, Esc only clears them (SearchBox),
// so the page looks at the key before the search does (capture), while the words are still there.
const back = () => void router.push({ name: 'events', query: { q: route.query.q } });
const onEsc = (e: KeyboardEvent) => {
  if (e.target instanceof HTMLInputElement && e.target.value) return;
  back();
};
</script>

<template>
  <section v-if="!id" class="flex min-h-0 flex-1 flex-col overflow-hidden" data-board>
    <PageTabs group="signals"><template v-if="catalog.events.length" #end><SearchBox v-model="query" size="sm" class="w-[320px] max-w-full" placeholder="Search events…" label="Search events" /></template></PageTabs>
    <div v-if="!catalog.events.length" class="flex min-h-0 min-w-0 flex-1 flex-col">
    <EmptyPage title="Nothing has come in yet" icon="inbox" say="Make an event signal named gitlab">
      <template #what>An event is one thing the outside told Remit: a post from a system (GitLab, GitHub, Sentry, Jira…) to an event signal's door, a line a script sent with <code class="code-chip">remit emit</code>, an issue or an alert Remit read itself by a sensor of the signal from its source. Each lands here as it came. What is done about it is a subscription to its signal: tell a chain's Chair, start a chain or run a job.</template>
      <template #how>
        <li>Ask the Assistant for an event signal for the system: it gets a door (<code class="code-chip">{{ ANY_PATH }}</code>) and a key, both on the signal's page on the Signals tab. A system with no way to post is read by a sensor instead: ask the Assistant to add one that reads its issues or alerts every few minutes.</li>
        <li>Give that system the address and the key, as the signal's guide shows, or post from a script with <code class="code-chip">remit emit</code>.</li>
        <li>Every event is listed here, with what each subscription did about it.</li>
      </template>
    </EmptyPage>
    </div>
    <template v-else>
      <div class="grid min-h-0 flex-1 auto-cols-[minmax(240px,1fr)] grid-flow-col gap-2.5 overflow-x-auto px-4 pt-2.5 pb-3.5">
        <EventList class="w-[400px] max-w-full rounded-lg bg-bg2 p-2" :rows="rows" :count="count" :hidden="hidden" @more="more" />
      </div>
      <p v-if="catalog.events.length >= EVENTS_KEPT" class="flex-none px-4 pb-2 text-center text-xs text-faint" data-kept>The newest {{ EVENTS_KEPT }} are listed.</p>
    </template>
  </section>
  <ListPage v-else name="events" open :busy="screen.switching" @keydown.esc.capture="onEsc">
    <template #tabs><PageTabs group="signals" /></template>
    <template #sidebar><EventsSidebar /></template>
    <EventPage v-if="event" :event="event" />
    <PageBody v-else><EmptyState role="status">{{ openedEvent.opening ? 'Opening the event…' : 'This event could not be opened.' }}</EmptyState></PageBody>
  </ListPage>
</template>
