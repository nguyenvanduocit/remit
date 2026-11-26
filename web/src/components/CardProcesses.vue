<script setup lang="ts">
import { computed, onMounted, onUnmounted, useId } from 'vue';
import { useApi } from '../core/api';
import { bytes } from '../ui';
import { useRemote } from '../useRemote';
import ListState from './ListState.vue';

const props = defineProps<{ cardId: string }>();
const api = useApi();
const remote = useRemote(() => props.cardId, (id) => api.cardProcesses(id));
const snapshot = computed(() => remote.of.value === props.cardId ? remote.data.value : undefined);
const headingId = useId();

let timer: ReturnType<typeof setInterval>;
onMounted(() => {
  timer = setInterval(() => {
    if (!document.hidden && !remote.loading.value) remote.retry();
  }, 2_000);
});
onUnmounted(() => clearInterval(timer));

function elapsed(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3_600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
  return `${Math.floor(seconds / 3_600)}h ${Math.floor(seconds / 60) % 60}m`;
}
</script>

<template>
  <section :aria-labelledby="headingId" class="max-w-5xl">
    <div class="mb-2 flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
      <h2 :id="headingId" class="caps">Processes</h2>
      <span v-if="snapshot?.shared" class="text-xs text-mut">Shared with the parent session</span>
    </div>
    <ListState what="the session processes" :held="!!snapshot" :empty="!snapshot?.processes.length" :error="remote.error.value" :retry="remote.retry">
      <template #empty>No live OS processes for this session.</template>
      <div v-if="snapshot" tabindex="0" role="region" :aria-labelledby="headingId" class="overflow-x-auto rounded-md border border-line focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-text">
        <table class="w-full min-w-[35rem] table-fixed text-left text-xs">
          <thead class="bg-panel text-text2">
            <tr>
              <th scope="col" class="px-2.5 py-2">Command</th>
              <th scope="col" class="w-16 px-2 py-2 text-right">PID</th>
              <th scope="col" class="w-20 px-2 py-2">State</th>
              <th scope="col" class="w-16 px-2 py-2 text-right" title="100% is one logical CPU core">CPU</th>
              <th scope="col" class="w-20 px-2 py-2 text-right" title="Resident memory">RAM</th>
              <th scope="col" class="w-[4.75rem] px-2.5 py-2 text-right">Elapsed</th>
            </tr>
          </thead>
          <tbody class="tabular-nums text-text2">
            <tr v-for="process in snapshot.processes" :key="process.pid" class="border-t border-line">
              <th scope="row" class="px-2.5 py-1.5 font-normal text-text">
                <div class="flex items-start gap-1.5" :style="{ paddingInlineStart: `${Math.min(process.depth, 5) * 0.5}rem` }">
                  <span v-if="process.depth" aria-hidden="true" class="text-mut">↳</span>
                  <span v-if="process.depth" class="sr-only">Process tree level {{ process.depth + 1 }}.</span>
                  <code class="min-w-0 [overflow-wrap:anywhere]">{{ process.command }}</code>
                </div>
              </th>
              <td class="px-2 py-1.5 text-right">{{ process.pid }}</td>
              <td class="px-2 py-1.5 [overflow-wrap:anywhere]">{{ process.state }}</td>
              <td class="whitespace-nowrap px-2 py-1.5 text-right">
                <template v-if="process.cpuPercent != null">{{ process.cpuPercent.toFixed(1) }}%</template>
                <span v-else aria-label="CPU sample unavailable">—</span>
              </td>
              <td class="whitespace-nowrap px-2 py-1.5 text-right">{{ bytes(process.residentBytes) }}</td>
              <td class="whitespace-nowrap px-2.5 py-1.5 text-right">{{ elapsed(process.elapsedSeconds) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </ListState>
  </section>
</template>
