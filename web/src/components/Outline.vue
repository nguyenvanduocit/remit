<script setup lang="ts">
import { computed } from 'vue';
import { elbows, ROW } from '../tree';

// An outline: its rows, one under the other in the order `rows` lists them (the slot draws each: AgentRow), and the lines that join each row to the one it hangs from (tree.ts).
const props = defineProps<{ rows: readonly { level: number; parent: number }[]; label: string }>();
const lines = computed(() => elbows(props.rows));
</script>

<template>
  <div class="relative">
    <svg class="lines" :height="rows.length * ROW" aria-hidden="true"><path v-for="(d, i) in lines" :key="i" :d="d" /></svg>
    <ul :aria-label="label"><slot /></ul>
  </div>
</template>

<style scoped>
.lines {
  position: absolute;
  inset: 0 auto auto 0;
  width: 100%;
  overflow: visible;
  pointer-events: none;
}
.lines path {
  fill: none;
  stroke: var(--tree-line);
  stroke-width: 1.5;
  stroke-linejoin: round;
}
</style>
