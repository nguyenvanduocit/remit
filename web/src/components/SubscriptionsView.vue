<script setup lang="ts">
import { computed } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { catalog, chainRecords } from '../catalog';
import type { SubscriptionKind, SubscriptionLine } from '../core/generated/types.generated';
import { useSubscriptionColumns } from '../subscriptionColumns';
import { useSignalDetail } from '../signalDetail';
import { fullTime, timed } from '../ui';
import Ago from './Ago.vue';
import KeyValues from './KeyValues.vue';
import LineWhen from './LineWhen.vue';
import ListPage from './ListPage.vue';
import NearMissesSection from './NearMissesSection.vue';
import Notice from './Notice.vue';
import PageBody from './PageBody.vue';
import PageSection from './PageSection.vue';
import PageTabs from './PageTabs.vue';
import SignalLink from './SignalLink.vue';
import SignalRecords from './SignalRecords.vue';
import SignalsFilter from './SignalsFilter.vue';
import SubscriptionActions from './SubscriptionActions.vue';
import SubscriptionBadge from './SubscriptionBadge.vue';
import SubscriptionColumn from './SubscriptionColumn.vue';
import SubscriptionHoldSection from './SubscriptionHoldSection.vue';
import SubscriptionsSidebar from './SubscriptionsSidebar.vue';
import SubscriptionKindIcon from './SubscriptionKindIcon.vue';
import WatchFacts from './WatchFacts.vue';
import PageGrid from './PageGrid.vue';

// The subscriptions of the signal backbone (ADR-0014), as the server lays them out (catalog.signals), in the board's shape; the
// signals they listen to have their own tab (SignalsView), and the events and what came of each have theirs (EventsView). With
// nothing open, the page is the board: the search and the kind, then every subscription (a waiting chain's check-back or watch,
// each routine, each event signal with the rules that place its events) as a card in one column per server group, side by side,
// each a page at a time. A card opens its line: the address names it, the board folds to one column beside it
// (SubscriptionsSidebar, ?col=), and the page says what it waits for and why it is held, with the newest readings of its signal
// and its firings, read from the server when it opens (signalDetail.ts). Why it does not fire though it would shows once, at the top: the
// firing it is held on with its error (whose words `held` also carries), else why it is held, else what the delivery of its next
// firing waits on; the readings it judged and did not fire on stand under what it does. An event signal's settings (its door, key and sensors) are on
// its page on the Signals tab. The buttons are the owner's rails: wake a Chair now, pause or resume its chain, a routine or an event signal,
// run a routine now. A routine and an event signal are made, changed and deleted by telling the Assistant, and a
// check-back or a watch is moved by telling the chain's Chair: never filled in here.
const route = useRoute();
const router = useRouter();
const board = computed(() => catalog.signals);
const subscriptions = computed(() => board.value?.subscriptions ?? []);
const titles = computed(() => new Map(chainRecords.value.map((c) => [c.id, c.title])));

const named = computed(() => (typeof route.params.entry === 'string' ? route.params.entry : ''));
const line = computed<SubscriptionLine | undefined>(() => subscriptions.value.find((l) => l.id === named.value));
const address = computed(() => line.value?.id);

// What changes what the open one adds (a reading came, it fired, its state moved): it is read again then.
const version = computed(() => (line.value ? [line.value.state, line.value.lastFired, line.value.lastRead].join('|') : ''));
const { detail, status } = useSignalDetail(address, version);

const { columns, query, kinds, kindCounts, more } = useSubscriptionColumns();

// Esc on the open line's page goes back to the whole board; in a search with words in it, Esc only clears them (SearchBox), so
// the page looks at the key before the search does (capture), while the words are still there.
const back = () => void router.push({ name: 'subscriptions', query: { q: route.query.q, kind: route.query.kind, hide: route.query.hide } });
const onEsc = (e: KeyboardEvent) => {
  if (e.target instanceof HTMLInputElement && e.target.value) return;
  back();
};

// What the open one's first card is about, and what its time means there.
const ABOUT: Record<SubscriptionKind, string> = { check_back: 'What the Chair checks', watch: 'What it watches for', routine: 'What it does', event: 'What it does with an event', automation: 'What it does' };
const WHEN: Record<SubscriptionKind, string> = { check_back: 'Checks back', watch: 'Wakes', routine: 'Next run', event: 'Hears', automation: 'Next time' };
</script>

<template>
  <section v-if="!line" class="flex min-h-0 flex-1 flex-col overflow-hidden" data-board>
    <PageTabs group="signals"><template #end><SignalsFilter v-model:query="query" v-model:kinds="kinds" :kind-counts="kindCounts" class="w-[400px] max-w-full" /></template></PageTabs>
    <div class="grid min-h-0 flex-1 auto-cols-[minmax(240px,1fr)] grid-flow-col gap-2.5 overflow-x-auto px-4 pt-2.5 pb-3.5">
      <SubscriptionColumn v-for="col in columns" :key="col.key" class="rounded-lg bg-bg2 p-2" :column="col" @more="more(col.key)" />
    </div>
  </section>
  <ListPage v-else name="subscriptions" open :title="line.title" @keydown.esc.capture="onEsc">
    <template #tabs><PageTabs group="signals" /></template>
    <template #sidebar><SubscriptionsSidebar /></template>
    <template #standing>
      <span data-badges class="flex flex-none items-center gap-1.5"><SubscriptionBadge :line="line" data-state /></span>
    </template>
    <template #actions><SubscriptionActions :line="line" /></template>
    <PageBody>
      <!-- The cards fill the page; wide enough (two cards of a readable width), they stand two abreast, each its own height. -->
      <div class="@container">
      <PageGrid>
        <SubscriptionHoldSection v-if="line.hold" class="@[1100px]:col-span-2" :hold="line.hold" />
        <Notice v-else-if="line.held" class="@[1100px]:col-span-2" data-held>{{ line.held }}</Notice>
        <Notice v-else-if="line.waiting" class="@[1100px]:col-span-2" tone="info" icon="clock" data-waiting>Its next firing waits: {{ line.waiting }}</Notice>
        <div class="flex min-w-0 flex-col gap-5">
        <PageSection :title="ABOUT[line.kind]" id="about" data-about>
          <template #aside><SubscriptionKindIcon :kind="line.kind" labelled /></template>
          <p class="mb-3.5 text-text2 [overflow-wrap:anywhere]" data-what>{{ line.what }}</p>
          <KeyValues>
            <dt>{{ WHEN[line.kind] }}</dt><dd data-when><LineWhen :line="line" /></dd>
            <WatchFacts v-if="line.kind === 'watch'" :line="line" />
            <template v-else><dt>Signal</dt><dd data-key><SignalLink :signal-key="line.key" :source="line.source" /></dd></template>
            <template v-if="line.kind === 'event' && line.condition"><dt>When</dt><dd class="mono [overflow-wrap:anywhere]" data-condition>{{ line.condition }}</dd></template>
            <template v-if="line.screen"><dt>Worth a chain when</dt><dd class="[overflow-wrap:anywhere]" data-screen>{{ line.screen }}</dd></template>
            <template v-if="line.lastFired"><dt>Last fired</dt><dd data-fired :title="fullTime(line.lastFired)"><Ago :at="line.lastFired" /></dd></template>
            <template v-if="timed(line.lastRead)"><dt>Last reading</dt><dd data-read :title="fullTime(line.lastRead!)"><Ago :at="line.lastRead!" /></dd></template>
            <template v-if="line.chainId">
              <dt>{{ line.kind === 'check_back' || line.kind === 'watch' ? 'Chain' : 'Run at work' }}</dt>
              <dd><RouterLink class="hit text-blue-text hover:underline" :to="{ name: 'chain', params: { id: line.chainId } }" data-chain>{{ titles.get(line.chainId) ?? line.chainId }}</RouterLink></dd>
            </template>
          </KeyValues>
        </PageSection>
        <NearMissesSection v-if="line.nearMisses?.length" :misses="line.nearMisses" />
        </div>
        <SignalRecords :detail="detail" :status="status" />
      </PageGrid>
      </div>
    </PageBody>
  </ListPage>
</template>
