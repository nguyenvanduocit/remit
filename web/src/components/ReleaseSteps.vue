<script setup lang="ts">
import type { ReleaseStep } from '../core/generated/types.generated';
import Ago from './Ago.vue';
import StandingBadge from './StandingBadge.vue';
import type { Look } from '../ui';

// The steps of a release's procedure, or of its way back, as they ran: each file's name, how it exited, when, and what it
// printed (scrubbed and cut by the server). A step still running has no exit and no end; one with an end and no exit could
// not be started.
defineProps<{ title: string; steps: ReleaseStep[] }>();
const lookOf = (s: ReleaseStep): Look & { label: string; working?: boolean } => {
  if (s.exit === undefined) return s.endedAt ? { tone: 'danger', icon: 'alert', label: 'Did not start' } : { tone: 'info', label: 'Running', working: true };
  return s.exit === 0 ? { tone: 'success', icon: 'check', label: 'Exited 0' } : { tone: 'danger', icon: 'alert', label: `Exited ${s.exit}` };
};
</script>

<template>
  <div class="flex flex-col gap-2 border-t border-line pt-3">
    <h3 class="text-[12.5px] font-semibold text-text">{{ title }}</h3>
    <ol class="m-0 flex list-none flex-col gap-2.5 p-0">
      <li v-for="(s, i) in steps" :key="`${i}-${s.name}`" class="flex flex-col gap-1" :data-step="s.name">
        <p class="m-0 flex flex-wrap items-center gap-x-2 gap-y-1 text-[12.5px]">
          <span class="mono text-text">{{ s.name }}</span>
          <StandingBadge :variant="lookOf(s).tone" :icon="lookOf(s).icon" :working="lookOf(s).working">{{ lookOf(s).label }}</StandingBadge>
          <span class="text-mut"><Ago :at="s.endedAt ?? s.startedAt" /></span>
        </p>
        <pre v-if="s.output" class="output">{{ s.output }}</pre>
      </li>
    </ol>
  </div>
</template>

<style scoped>
.output {
  margin: 0;
  padding: 7px 8px;
  border: 1px solid var(--line);
  border-radius: 7px;
  background: var(--bg);
  font-family: var(--mono);
  font-size: 11px;
  color: var(--text2);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
</style>
