<script setup lang="ts">
import type { NearMiss } from '../core/generated/types.generated';
import { SKIP_WORDS } from '../subscriptionWords';
import { fullTime } from '../ui';
import Ago from './Ago.vue';
import PageSection from './PageSection.vue';

// The newest readings an open subscription judged and did not fire on, as the server keeps them (at most three, the newest first),
// as a card of its page: why in words, the key it read, when it judged it, and for a condition that could not be read the error,
// as text (it names what the reading held, so it is never read as Markdown).
defineProps<{ misses: NearMiss[] }>();
</script>

<template>
  <PageSection title="Judged and did not fire" :count="misses.length" id="near-misses" data-near-misses>
    <template #aside>Newest first</template>
    <ul class="m-0 flex list-none flex-col p-0">
      <li v-for="m in misses" :key="m.seq" class="flex flex-col gap-1 border-b border-line py-2 first:pt-0 last:border-b-0 last:pb-0" data-near-miss>
        <div class="flex flex-wrap items-baseline gap-x-2 gap-y-0.5 text-[12.5px]">
          <b class="font-semibold text-text2" data-reason>{{ SKIP_WORDS[m.reason] }}</b>
          <span class="mono min-w-0 text-mut [overflow-wrap:anywhere]" data-key>{{ m.key }}</span>
          <span class="ml-auto flex-none text-[12px] text-mut tabular-nums" :title="fullTime(m.at)" data-at><Ago :at="m.at" /></span>
        </div>
        <pre v-if="m.detail" class="detail" data-detail>{{ m.detail }}</pre>
      </li>
    </ul>
  </PageSection>
</template>

<style scoped>
.detail {
  margin: 0;
  color: var(--text2);
  font-family: var(--mono);
  font-size: 11.5px;
  line-height: 1.45;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
</style>
