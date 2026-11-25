<script setup lang="ts">
import type { Hold } from '../core/generated/types.generated';
import { HOLD_WORDS } from '../subscriptionWords';
import { fullTime } from '../ui';
import Ago from './Ago.vue';
import KeyValues from './KeyValues.vue';
import PageSection from './PageSection.vue';

// The firing an open subscription is held in place on, failed for good, as a card of its page: why (its reason in words, the
// heading), since when, the firing (its key and revision) and the error its processor last answered. The error came from the
// processor, so it is text in a box of its own, never read as Markdown. The subscription waits, losing no reading, until the
// firing is retried or given up: that is the owner's word to the Assistant, or his answer to the item in his inbox.
defineProps<{ hold: Hold }>();
</script>

<template>
  <PageSection :title="HOLD_WORDS[hold.reason]" id="hold" data-hold>
    <template #aside><span :title="fullTime(hold.since)" data-since>Held <Ago :at="hold.since" /></span></template>
    <KeyValues>
      <dt>Firing</dt><dd data-firing-key><span class="mono">{{ hold.firing.key }}</span> · rev {{ hold.firing.rev }}</dd>
      <template v-if="hold.detail"><dt>Error</dt><dd><pre class="error" tabindex="0" aria-label="The error its processor answered, shown as text" data-detail>{{ hold.detail }}</pre></dd></template>
    </KeyValues>
    <p class="mt-3.5 text-[12.5px] leading-[1.45] text-text2">It waits here, losing no reading, until the firing is retried or given up. Tell the Assistant which, or answer it in your inbox.</p>
  </PageSection>
</template>

<style scoped>
.error {
  max-height: 11rem;
  margin: 0;
  padding: 7px 9px;
  overflow: auto;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--bg2);
  color: var(--text2);
  font-family: var(--mono);
  font-size: 11.5px;
  line-height: 1.45;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
</style>
