<script setup lang="ts">
import { computed } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { catalog } from '../catalog';
import type { SignalLine } from '../core/generated/types.generated';
import { matchesKey } from '../pattern';
import { patternOf, useSignalList } from '../signalList';
import { useSignalDetail } from '../signalDetail';
import { useSignalRows } from '../signalRows';
import { timed } from '../ui';
import Ago from './Ago.vue';
import KeyValues from './KeyValues.vue';
import ListeningSubscriptions from './ListeningSubscriptions.vue';
import ListPage from './ListPage.vue';
import Notice from './Notice.vue';
import PageBody from './PageBody.vue';
import PageGrid from './PageGrid.vue';
import PageSection from './PageSection.vue';
import PageTabs from './PageTabs.vue';
import SignalRecords from './SignalRecords.vue';
import SignalRowsTable from './SignalRowsTable.vue';
import SignalSearch from './SignalSearch.vue';
import SignalsSidebar from './SignalsSidebar.vue';
import StandingBadge from './StandingBadge.vue';
import EventSignalRail from './EventSignalRail.vue';
import EventSignalSettings from './EventSignalSettings.vue';

// The signals of the signal backbone (ADR-0014): what Remit can hear. With nothing open, the page is one table of every
// stored row, each reading and each firing, newest first, picked by a key pattern (signalRows.ts): the key shapes the server knows
// (time, events, a chain's landings, its change being live, its sensors, Remit's own facts) are patterns like
// any other, and typed exactly one says what its signals are. What subscribes to them is on the Subscriptions tab. A signal opens its page: its newest reading, for an event signal its settings (EventSignalSettings: its door's
// address, its sensors, its key) with pause and resume as the owner's rails, the subscriptions that listen
// to it (the signal's own rules among them), then its readings, each an event with a link to what became of it, and the
// firings on it, read from the server when it opens (signalDetail.ts), and the key shape it has, which narrows the list to its kin.
// The page has the list beside it (SignalsSidebar). Nothing here is edited: a signal is what a source said.
const route = useRoute();
const router = useRouter();
const board = computed(() => catalog.signals);
const named = computed(() => (typeof route.params.entry === 'string' ? route.params.entry : ''));
const signal = computed<SignalLine | undefined>(() => board.value?.signals.find((s) => s.id === named.value));
// The event signal the open signal is, whose settings its page shows.
const eventSignal = computed(() => (signal.value?.eventSignalId ? catalog.eventSignals.find((s) => s.id === signal.value?.eventSignalId) : undefined));
const signalSource = computed(() => board.value?.sources.find((s) => s.source === signal.value?.source));
// The subscriptions on the Subscriptions tab that listen to the open signal: those whose pattern (ADR-0049) its key matches.
const listening = computed(() => board.value?.subscriptions.filter((l) => !!l.key && !!signal.value && matchesKey(l.key, signal.value.key)) ?? []);

// A new reading changes what the open signal adds: it is read again then.
const { detail, status } = useSignalDetail(
  computed(() => signal.value?.id),
  computed(() => String(signal.value?.rev)),
);
const { query, picked, sources, total } = useSignalList();
const table = useSignalRows(query, computed(() => String(board.value?.rev)));

// Esc on an open page goes back to the whole board.
const back = () => void router.push({ name: 'signals', query: { q: route.query.q } });
</script>

<template>
  <section v-if="!signal" class="flex min-h-0 flex-1 flex-col overflow-hidden" data-board>
    <!-- The search sits a hair inside the strip, as on the board (BoardView), so the list starts right under the tabs. -->
    <PageTabs group="signals"><template #end><SignalSearch v-model:query="query" :sources="sources" :total="total" class="w-[400px] max-w-full" /></template></PageTabs>
    <div class="min-h-0 flex-1 overflow-auto px-4 pt-2.5 pb-3.5">
      <div v-if="picked" class="mb-2.5 max-w-4xl text-xs text-mut" data-picked>
        <p class="[overflow-wrap:anywhere]" data-what>{{ picked.what }}</p>
        <p class="mt-1 [overflow-wrap:anywhere]" data-who>Who listens: {{ picked.who }}</p>
      </div>
      <SignalRowsTable class="rounded-lg bg-bg2" :rows="table.rows.value" :status="table.status.value" :full="table.full.value" @more="table.more" />
    </div>
  </section>
  <ListPage v-else name="signals" open :title="signal.title" @keydown.esc="back">
    <template #tabs><PageTabs group="signals" /></template>
    <template #sidebar><SignalsSidebar /></template>
    <template v-if="eventSignal?.paused" #standing><StandingBadge variant="quiet" icon="pause" class="flex-none" data-paused>Paused</StandingBadge></template>
    <template v-if="eventSignal" #actions><EventSignalRail :id="eventSignal.id" :name="eventSignal.name" :paused="!!eventSignal.paused" /></template>
    <PageBody>
      <!-- The cards fill the page; wide enough (two cards of a readable width), they stand two abreast, each its own height. -->
      <div class="@container">
        <PageGrid>
          <PageSection title="The signal" id="signal" data-signal>
            <template v-if="signalSource" #aside><RouterLink class="font-mono text-[12px] text-blue-text hover:underline" :to="{ name: 'signals', query: { q: patternOf(signalSource) } }" :title="`Every signal of ${signalSource.title}`" data-pattern-link>{{ patternOf(signalSource) }}</RouterLink></template>
            <KeyValues>
              <dt>Signal</dt><dd class="mono" data-key>{{ signal.key }}</dd>
              <dt>Newest reading</dt>
              <dd data-newest><template v-if="signal.rev">rev {{ signal.rev }}<template v-if="timed(signal.at)"> · <Ago :at="signal.at!" /></template><template v-if="signal.cause"> · {{ signal.cause }}</template></template><template v-else>none yet</template></dd>
            </KeyValues>
            <Notice v-if="signal.silent" class="mt-3.5" data-silent>{{ signal.silent }}</Notice>
          </PageSection>
          <EventSignalSettings v-if="eventSignal" :key="eventSignal.id" :event-signal="eventSignal" :silent-sensors="signal.silentSensors" />
          <ListeningSubscriptions :lines="listening" :events="signal.source === 'event'" />
          <SignalRecords :detail="detail" :status="status" />
        </PageGrid>
      </div>
    </PageBody>
  </ListPage>
</template>
