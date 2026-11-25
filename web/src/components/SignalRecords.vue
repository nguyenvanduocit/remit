<script setup lang="ts">
import { ref, watch } from 'vue';
import { clockAt } from '../ui';
import type { SignalDetail } from '../core/generated/types.generated';
import type { DetailStatus } from '../signalDetail';
import EmptyState from './EmptyState.vue';
import ListState from './ListState.vue';
import FiringItem from './FiringItem.vue';
import Icon from './Icon.vue';
import Notice from './Notice.vue';
import PageSection from './PageSection.vue';
import ShowMore from './ShowMore.vue';

// What one open line adds to the board, read from the server when it opens: the newest readings of its signal and the firings
// made, a card each in the page's grid: the firings beside the line's own card where the page is wide, the readings (JSON, wide)
// across the whole page under them, each showing a page at a time (a signal read every minute has hundreds of each). A reading of
// an event signal links to what became of that event, its firings on the Events tab. A reading's value came from outside Remit, so it is text and only text: cut where the server cut it, in
// a box of its own (never read as Markdown or HTML), and the card says so.
const props = defineProps<{ detail: SignalDetail | null; status: DetailStatus }>();
const READINGS = 10;
const FIRINGS = 25;
const readingsShown = ref(READINGS);
const firingsShown = ref(FIRINGS);
// Another line opened: its pages start over.
watch(
  () => props.detail,
  () => {
    readingsShown.value = READINGS;
    firingsShown.value = FIRINGS;
  },
);
</script>

<template>
  <!-- Its cards stand in the page's grid as if written there (contents), two abreast where the page is wide. -->
  <div class="contents">
    <Notice v-if="status === 'ended'" tone="quiet" icon="circle" data-ended>It has fired and ended since the list was made, so there is nothing more to read of it.</Notice>
    <ListState v-else-if="status === 'failed' || status === 'loading'" what="this signal" :held="false" :error="status === 'failed' ? 'it is read again when the link is back' : undefined" />
    <template v-else-if="detail">
      <PageSection title="Firings" :count="detail.firings.length" id="firings" data-firings>
        <template #aside>Newest first</template>
        <EmptyState v-if="!detail.firings.length">It has not fired yet: a firing shows here each time a subscription acts on one of its readings.</EmptyState>
        <ul v-else class="m-0 flex list-none flex-col p-0"><FiringItem v-for="(f, i) in detail.firings.slice(0, firingsShown)" :key="i" class="border-b border-line py-1.5 last:border-b-0" :firing="f" /></ul>
        <ShowMore v-if="detail.firings.length > firingsShown" class="mt-3" @click="firingsShown += FIRINGS">Show {{ Math.min(FIRINGS, detail.firings.length - firingsShown) }} more of {{ detail.firings.length - firingsShown }}</ShowMore>
      </PageSection>
      <PageSection v-if="detail.key" title="Newest readings" :count="detail.readings.length" id="readings" class="@[1100px]:col-span-2" data-readings>
        <template #aside>Data from outside Remit, shown as text only.</template>
        <EmptyState v-if="!detail.readings.length">No reading yet: its writers (a sensor, its door, an agent) fill it.</EmptyState>
        <ol v-else class="m-0 flex list-none flex-col gap-3 p-0">
          <li v-for="r in detail.readings.slice(0, readingsShown)" :key="r.rev" data-reading>
            <div class="mb-1 flex flex-wrap items-baseline gap-x-2 text-[12px] text-mut"><b class="font-semibold text-text2">rev {{ r.rev }}</b><span>{{ clockAt(r.at) }}</span><span class="[overflow-wrap:anywhere]">{{ r.cause }}</span><RouterLink v-if="r.eventId" class="ml-auto inline-flex flex-none items-center gap-1 text-blue-text hover:underline" :to="{ name: 'events', params: { id: r.eventId } }" :aria-label="`What became of it: the event of reading ${r.rev}`" data-event-link>What became of it<Icon name="arrow-right" :size="11" /></RouterLink></div>
            <pre class="value mono" tabindex="0" :aria-label="`Value of reading ${r.rev}, shown as text`">{{ r.value }}</pre>
            <p v-if="r.truncated" class="mt-1 text-[11.5px] text-mut" data-cut>Cut here: the reading is longer.</p>
          </li>
        </ol>
        <ShowMore v-if="detail.readings.length > readingsShown" class="mt-3" @click="readingsShown += READINGS">Show {{ Math.min(READINGS, detail.readings.length - readingsShown) }} more of {{ detail.readings.length - readingsShown }}</ShowMore>
      </PageSection>
    </template>
  </div>
</template>

<style scoped>
.value {
  max-height: 11rem;
  margin: 0;
  padding: 8px 10px;
  overflow: auto;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--bg2);
  color: var(--text2);
  font-size: 11.5px;
  line-height: 1.45;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
</style>
