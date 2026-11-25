<script setup lang="ts">
import { computed } from 'vue';
import type { Freshness } from '../core/generated/replica/types.generated';
import Icon from './Icon.vue';
import Spinner from './Spinner.vue';

// How current what a feed shows is (ADR-0095 decision 11), above the feed, said as the link's banner says it: nothing
// while it is live; while it is read again (a reconnect, its home let go by the server, an opening), that it is being
// brought up to date; while stale, since when what shows dates. The banner under the top bar speaks for the whole page;
// this speaks for the one feed, which may be read again while the rest is live.
const props = defineProps<{ status: Freshness }>();
const clock = (ms: number) => new Date(ms).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
const text = computed(() => {
  const s = props.status;
  if (s.status === 'repairing') return 'Bringing this up to date…';
  if (s.status === 'stale') return `Shown as of ${clock(s.since)}: ${s.reason === 'offline' ? 'not connected' : 'could not be brought up to date'}.`;
  return '';
});
</script>

<template>
  <div v-if="text" role="status" class="flex flex-none items-center gap-1.5 border-b px-4 py-1 text-[11.5px]" :class="status.status === 'stale' ? 'border-amber-line bg-amber-bg text-amber' : 'border-blue-line bg-blue-bg text-blue-hi'" :data-freshness="status.status">
    <Spinner v-if="status.status === 'repairing'" />
    <Icon v-else name="alert" :size="11" class="flex-none" />
    <span class="min-w-0">{{ text }}</span>
  </div>
</template>
