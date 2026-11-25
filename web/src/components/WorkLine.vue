<script setup lang="ts">
import { computed } from 'vue';
import type { DoingTone } from '../core/generated/types.generated';
import { useClock } from '../clock';
import { doingText, span } from '../ui';
import LiveDot from './LiveDot.vue';

// What an agent does right now, as the server words it (AgentStanding, Seat): its work (the plan step in progress, else its
// brief's title), then how long it has stood so since `since`, then `doing` (the tool in use, or what it waits on) in the
// colour of its tone. While its time counts as work (`warms`) a dot pings before it; on a wait with a known reason it is
// still. `lead` puts a word before the line where the line stands alone (a seat's card: "Now"). The time is a leaf on the
// clock, so it alone re-renders.
const props = defineProps<{ work: string; since?: string; warms: boolean; doing: string; doingTone: DoingTone; lead?: string; wrap?: boolean }>();
const now = useClock();
const time = computed(() => (props.since ? span(now.value.getTime() - new Date(props.since).getTime()) : ''));
</script>

<template>
  <span class="flex min-w-0 items-center gap-1.5" data-work-line :data-warms="warms || undefined">
    <LiveDot v-if="warms" class="flex-none text-cyan-text" live />
    <span class="min-w-0" :class="wrap ? '[overflow-wrap:anywhere]' : 'truncate'">
      <span v-if="lead" class="font-medium text-mut">{{ lead }}: </span>
      <span v-if="work" class="text-text2" data-work>{{ work }}</span>
      <template v-if="time"><span v-if="work" aria-hidden="true"> · </span><span class="tabular-nums text-mut" data-since :title="since">{{ time }}</span></template>
      <template v-if="doing"><span v-if="work || time" aria-hidden="true"> · </span><span :class="doingText(doingTone) || 'text-mut'" data-doing>{{ doing }}</span></template>
    </span>
  </span>
</template>
