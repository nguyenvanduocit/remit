<script setup lang="ts">
import { computed } from 'vue';
import { catalog, standingOf, chainRecords } from '../catalog';
import { clockAt } from '../ui';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import EmptyState from './EmptyState.vue';
import LiveDot from './LiveDot.vue';

// What the scheduler is doing: how many chains run (of the limit the server measured, and what presses on it). The running count is a button that opens which chains run, wait
// (and when the next check-back is due) and are queued, each a link (a popover, as the check-back chip's: the keyboard
// reaches its links).
const workload = computed(() => catalog.workload);
const running = computed(() => chainRecords.value.filter((c) => c.status === 'running'));
const queued = computed(() => chainRecords.value.filter((c) => c.status === 'queued'));
const waiting = computed(() => chainRecords.value.filter((c) => c.status === 'waiting'));
// When the server's next check-back is due: the earliest wake a waiting chain holds. A chain that waits on people's replies alone
// holds none, so with only such chains the server sends none and nothing here claims a check-back.
const nextCheckBack = computed(() => {
  const at = workload.value?.nextCheckBack;
  return at ? clockAt(at) : '';
});
</script>

<template>
  <div v-if="workload" class="pill flex items-center gap-3.5 rounded-2xl border border-line bg-panel px-3 text-xs text-mut max-[1240px]:gap-2.5">
    <!-- The popover lists the chains behind the running count, so it hangs from it and not from the whole pill. -->
    <Popover>
      <PopoverTrigger as-child>
        <button type="button" class="counts flex cursor-pointer items-center py-[5px]">
          <span><LiveDot :live="!!workload.running" :class="workload.running ? 'text-info' : 'text-faint'" /><b>{{ workload.running }}</b>/{{ workload.capacity }} running</span>
        </button>
      </PopoverTrigger>
      <PopoverContent class="w-84" align="start">
        <div class="flex flex-col gap-3 text-[12.5px]">
          <template v-for="[label, list, note] in ([['Running', running, ''], ['Waiting', waiting, nextCheckBack && `Next check-back at ${nextCheckBack}`], ['Queue', queued, '']] as const)" :key="label">
            <section v-if="list.length">
              <h2 class="caps mb-1.5">{{ label }}</h2>
              <p v-if="note" class="m-0 mb-1.5 text-[11.5px] text-mut" data-waiting-note>{{ note }}</p>
              <div class="flex flex-col gap-0.5">
                <RouterLink v-for="c in list" :key="c.id" class="chain" :to="{ name: 'chain', params: { id: c.id } }">
                  <LiveDot :class="c.status === 'running' ? 'text-info' : 'text-faint'" />
                  <span class="truncate">{{ c.title }}</span>
                  <small>{{ c.kind }} · from {{ standingOf(c.id).from }}</small>
                </RouterLink>
              </div>
            </section>
          </template>
          <EmptyState v-if="!running.length && !waiting.length && !queued.length">No chains are running or waiting.</EmptyState>
          <p v-if="workload.pressure" class="m-0 text-[11.5px] text-text2" data-pressure><b>Limit held down:</b> {{ workload.pressure }}.</p>
          <p class="m-0 border-t border-line pt-2.5 text-[11.5px] text-mut">Remit runs on its own: queued chains start when there is room, and the limit of {{ workload.capacity }} grows by one each look while nothing presses on the accounts or the host; routines start chains at their times. <RouterLink class="text-blue-text hover:underline" :to="{ name: 'subscriptions' }">See what it waits for</RouterLink></p>
        </div>
      </PopoverContent>
    </Popover>
  </div>
</template>

<style scoped>
.pill span {
  display: flex;
  align-items: center;
  gap: 5px;
  white-space: nowrap;
}
.counts {
  border-radius: 6px;
  color: inherit;
}
.counts:hover b {
  color: var(--blue-text);
}
.pill b {
  font-weight: 600;
  color: var(--text);
}
.chain {
  display: grid;
  grid-template-columns: 7px 1fr;
  align-items: center;
  column-gap: 9px;
  margin: 0 -8px;
  padding: 6px 8px;
  border-radius: 6px;
  color: var(--text);
  text-decoration: none;
}
.chain:hover {
  background: var(--panel3);
}
.chain small {
  grid-column: 2;
  font-size: 11px;
  color: var(--mut);
}
</style>
