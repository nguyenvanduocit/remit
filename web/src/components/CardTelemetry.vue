<script setup lang="ts">
import { computed, useId } from 'vue';
import { useApi } from '../core/api';
import { useRemote } from '../useRemote';
import { plural, tokens } from '../ui';
import type { Card } from '../core/generated/types.generated';
import ListState from './ListState.vue';
import ContextList from './ContextList.vue';

// What the agent did, in its Session panel, as its Claude Code sessions exported it (ADR-0085): what its model requests
// cost and how fast they answered, which failed, where its tokens went, and how each tool ran. Asked while the panel shows,
// and again as each turn starts and ends.
const props = defineProps<{ card: Card }>();
const api = useApi();
const remote = useRemote(
  () => ({ cardId: props.card.id, turns: props.card.turns, status: props.card.status }),
  ({ cardId }) => api.cardTelemetry(cardId),
);
const t = computed(() => (remote.of.value?.cardId === props.card.id ? remote.data.value : undefined));
const id = useId();

const usd = (n: number) => (n >= 1 ? `$${n.toFixed(2)}` : n >= 0.01 ? `$${n.toFixed(3)}` : `$${n.toFixed(4)}`);
const ms = (n: number) => (n >= 60_000 ? `${(n / 60_000).toFixed(1)} min` : n >= 1000 ? `${(n / 1000).toFixed(1)} s` : `${n} ms`);
const at = (msSinceEpoch: number) => new Date(msSinceEpoch).toLocaleString();

// When it worked: the day once, then the two times, and how long between them.
const span = computed(() => {
  if (!t.value?.firstAtMs) return '';
  const a = new Date(t.value.firstAtMs);
  const b = new Date(t.value.lastAtMs);
  const time = (d: Date) => d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  const day = (d: Date) => d.toLocaleDateString([], { month: 'short', day: 'numeric' });
  const range = a.toDateString() === b.toDateString() ? `${day(a)}, ${time(a)}–${time(b)}` : `${day(a)} ${time(a)} – ${day(b)} ${time(b)}`;
  return `${range} · ${ms(t.value.lastAtMs - t.value.firstAtMs)}`;
});

// Where the tokens went, as shares of one bar: read from cache, written to cache, new input, output.
const tokenParts = computed(() => {
  const x = t.value;
  if (!x) return [];
  const parts = [
    { label: 'Cache read', n: x.cacheReadTokens, tone: 'bg-blue/70' },
    { label: 'Cache write', n: x.cacheCreationTokens, tone: 'bg-violet/80' },
    { label: 'New input', n: x.inputTokens, tone: 'bg-amber' },
    { label: 'Output', n: x.outputTokens, tone: 'bg-green' },
  ];
  const total = parts.reduce((s, p) => s + p.n, 0);
  return parts.map((p) => ({ ...p, share: total ? p.n / total : 0 }));
});
const tokenTotal = computed(() => tokenParts.value.reduce((s, p) => s + p.n, 0));
const toolMax = computed(() => Math.max(1, ...(t.value?.tools.map((u) => u.totalMs) ?? [])));
</script>

<template>
  <section :aria-labelledby="id" class="@container mt-6">
    <div class="mb-3 flex flex-wrap items-baseline justify-between gap-x-3 gap-y-0.5">
      <h2 :id="id" class="caps">Telemetry</h2>
      <span v-if="span" class="text-[11.5px] text-mut tabular-nums">{{ span }}</span>
    </div>
    <ListState what="the telemetry" :held="!!t" :error="remote.error.value" :empty="!!t && !t.requests && !t.errors && !t.tools.length" :retry="remote.retry">
    <template #empty>Nothing exported yet: the agent's session sends its telemetry as it works.</template>
    <template v-if="t">
      <dl class="grid grid-cols-2 gap-2 @[460px]:grid-cols-4">
        <div class="tile">
          <dt>Cost</dt>
          <dd><b>{{ usd(t.costUsd) }}</b><span>{{ plural(t.requests, 'request') }}</span></dd>
        </div>
        <div class="tile">
          <dt>Latency</dt>
          <dd><b>{{ ms(t.latencyP50Ms) }}</b><span>p50 · p95 {{ ms(t.latencyP95Ms) }}</span></dd>
        </div>
        <div class="tile">
          <dt>First token</dt>
          <dd><b>{{ ms(t.ttftP50Ms) }}</b><span>p50</span></dd>
        </div>
        <div class="tile" :class="{ bad: t.errors }">
          <dt>Failed</dt>
          <dd><b>{{ t.errors }}</b><span>of {{ plural(t.requests, 'request') }}</span></dd>
        </div>
      </dl>

      <div class="mt-4">
        <div class="mb-1.5 flex items-baseline justify-between text-[12.5px]">
          <span class="font-semibold text-text">Tokens</span>
          <span class="text-mut tabular-nums">{{ tokens(tokenTotal) }} total</span>
        </div>
        <div class="flex h-2 gap-px overflow-hidden rounded-full bg-panel3" aria-hidden="true">
          <i v-for="p in tokenParts" v-show="p.share > 0" :key="p.label" :class="p.tone" :style="{ flexGrow: p.share, minWidth: '3px' }" />
        </div>
        <ul class="mt-2 grid grid-cols-2 gap-x-4 gap-y-1 text-[12px]">
          <li v-for="p in tokenParts" :key="p.label" class="flex items-center gap-1.5">
            <i class="size-2 flex-none rounded-[2px]" :class="p.tone" aria-hidden="true" />
            <span class="text-mut">{{ p.label }}</span>
            <span class="ml-auto text-text2 tabular-nums">{{ tokens(p.n) }}</span>
          </li>
        </ul>
      </div>

      <div v-if="t.models.length" class="mt-4">
        <div class="mb-1 text-[12.5px] font-semibold text-text">Models</div>
        <ul class="rows">
          <li v-for="m in t.models" :key="`${m.model} ${m.effort}`" class="flex items-baseline gap-2">
            <span class="mono min-w-0 flex-1 truncate text-text2">{{ m.model }}<span v-if="m.effort" class="text-mut"> · {{ m.effort }}</span></span>
            <span class="text-mut tabular-nums">{{ plural(m.requests, 'request') }}</span>
            <span class="w-[64px] text-right text-text2 tabular-nums">{{ usd(m.costUsd) }}</span>
          </li>
        </ul>
      </div>

      <div class="mt-3">
        <ContextList title="Tools" :items="t.tools" :text="(u) => u.name" open>
          <template #default="{ item }">
            <div class="grid grid-cols-[minmax(0,1fr)_auto_auto] items-center gap-x-3 @[460px]:grid-cols-[minmax(0,1fr)_auto_96px_auto]">
              <span class="mono truncate text-text2">{{ item.name }}</span>
              <span class="text-right text-mut tabular-nums">{{ plural(item.runs, 'run') }}<span v-if="item.failed" class="text-red-text"> · {{ item.failed }} failed</span></span>
              <span class="hidden h-1.5 overflow-hidden rounded-full bg-panel3 @[460px]:block" aria-hidden="true"><i class="block h-full rounded-full bg-blue/60" :style="{ width: `${Math.max(2, (item.totalMs / toolMax) * 100)}%` }" /></span>
              <span class="w-[118px] text-right text-mut tabular-nums"><span class="text-text2">{{ ms(item.totalMs) }}</span> · p95 {{ ms(item.p95Ms) }}</span>
            </div>
          </template>
        </ContextList>
        <ContextList v-if="t.recentErrors.length" title="Failed requests" :items="t.recentErrors" :text="(e) => e.error" open>
          <template #default="{ item }">
            <span class="text-mut">{{ at(item.atMs) }}</span> <span class="mono">{{ item.model }}</span> <span v-if="item.status" class="text-red-text">{{ item.status }}</span> <span class="text-text2">{{ item.error }}</span>
          </template>
        </ContextList>
      </div>
    </template>
    </ListState>
  </section>
</template>

<style scoped>
.tile {
  min-width: 0;
  padding: 8px 10px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--panel);
}
.tile dt {
  color: var(--mut);
  font-size: 11px;
}
.tile dd {
  display: flex;
  flex-direction: column;
  margin: 2px 0 0;
}
.tile b {
  color: var(--text);
  font-size: 17px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  line-height: 1.25;
}
.tile dd span {
  overflow: hidden;
  color: var(--mut);
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tile.bad {
  border-color: var(--red-line);
  background: var(--red-bg);
}
.tile.bad b {
  color: var(--red-text);
}
.rows {
  margin: 0;
  padding: 0;
  list-style: none;
  font-size: 12px;
}
.rows > li {
  padding: 3px 0;
}
</style>
