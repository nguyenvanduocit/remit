<script setup lang="ts">
import { computed } from 'vue';
import { tokens } from '../ui';
import Tip from './Tip.vue';

// How full the agent's context window is, in one short line: the tokens, a strip of cells as wide as its place gives it
// up to a short line's width, the share; amber from 80%. It takes the width its place leaves (it is a container): too narrow
// for the tokens, it shows the cells and the share, the tip keeping the tokens. Pressing it shows what fills it.
const props = defineProps<{ used: number; size: number }>();
defineEmits<{ show: [] }>();
const CELLS = 16;
const share = computed(() => (props.size > 0 ? Math.min(1, props.used / props.size) : 0));
const lit = computed(() => Math.round(share.value * CELLS));
const left = computed(() => (share.value >= 0.8 ? ` · ${tokens(props.size - props.used)} left` : ''));
</script>

<template>
  <div v-if="size > 0" class="@container flex min-w-0 flex-1 justify-end">
    <Tip :text="`Context: ${tokens(used)} / ${tokens(size)} tokens${left} · what fills it`">
      <button type="button" class="meter" :class="{ warn: share >= 0.8 }" :aria-label="`Context ${tokens(used)} of ${tokens(size)}`" @click="$emit('show')">
        <span class="hidden text-[11px] text-mut @[176px]:inline"><b class="font-semibold text-text2">{{ tokens(used) }}</b>/{{ tokens(size) }}</span>
        <span class="cells"><i v-for="n in CELLS" :key="n" :class="{ on: n <= lit }"></i></span>
        <span class="pct">{{ Math.round(share * 100) }}%</span>
      </button>
    </Tip>
  </div>
</template>

<style scoped>
.meter {
  width: auto;
  max-width: 280px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 3px 6px;
  border: 1px solid var(--line);
  border-radius: 6px;
  background: var(--panel);
  cursor: pointer;
}
.meter:hover {
  border-color: var(--line2);
}
.pct {
  color: var(--text2);
  font-size: 10.5px;
  font-variant-numeric: tabular-nums;
}
.warn .pct {
  color: var(--amber);
}
/* The cells keep a least width of their own: in a column too narrow for them (a header with many tabs), the share is said
   by the figure alone, so the meter never runs past its column. */
.cells {
  display: none;
  flex: 1;
  grid-template-columns: repeat(16, minmax(3px, 1fr));
  gap: 1.5px;
}
@container (min-width: 120px) {
  .meter {
    width: 100%;
  }
  .cells {
    display: grid;
  }
}
.cells i {
  height: 8px;
  border-radius: 1px;
  background: var(--panel3);
}
.cells i.on {
  background: var(--meter-on);
}
.warn .cells i.on {
  background: var(--amber);
}
</style>
