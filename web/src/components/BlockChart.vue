<script setup lang="ts">
import { useElementSize } from '@vueuse/core';
import { computed, ref, useId } from 'vue';
import type { BlockPart } from '../core/generated/types.generated';

// A chart of a block (ADR-0091): its series as bars or lines in SVG drawn at the width it is given, a legend that tells the
// series apart by name and by the shape of its mark (a solid, a dashed, a dotted line), and its numbers as a table behind a
// toggle, the chart's text equivalent. Names are plain text.
type Chart = Extract<BlockPart, { type: 'chart' }>;
const props = defineProps<{ part: Chart }>();
const id = useId();
const box = ref<HTMLElement | null>(null);
const { width } = useElementSize(box);
const table = ref(false);

const H = 150;
const PAD = { top: 8, right: 8, bottom: 22, left: 40 };
const DASH = ['', '5 3', '1.5 3', '8 3 1.5 3'];

const num = (v: number) => (Number.isInteger(v) ? String(v) : v.toFixed(Math.abs(v) < 10 ? 2 : 1).replace(/\.?0+$/, ''));
const withUnit = (v: number) => num(v) + (props.part.unit ? ` ${props.part.unit}` : '');

// The scale runs from zero, or below it to the lowest value, up to zero or the highest: each end rounded away from zero to
// 1, 2, 2.5 or 5 times a power of ten, so its ticks read whole. Zero is always a tick, and a line of its own when the scale
// crosses it; a bar grows from zero, up or down.
const nice = (v: number) => {
  if (v === 0) return 0;
  const p = 10 ** Math.floor(Math.log10(v));
  return ([1, 2, 2.5, 5, 10].find((m) => m * p >= v) ?? 10) * p;
};
const values = computed(() => props.part.series.flatMap((s) => s.values));
const top = computed(() => nice(Math.max(0, ...values.value)));
const bottom = computed(() => -nice(-Math.min(0, ...values.value)));
const range = computed(() => top.value - bottom.value || 1);
const ticks = computed(() => {
  if (bottom.value === 0) return [0, range.value / 2, range.value];
  if (top.value === 0) return [bottom.value, bottom.value / 2, 0];
  return [bottom.value, 0, top.value];
});
const crosses = computed(() => bottom.value < 0 && top.value > 0);
const plotW = computed(() => Math.max(0, width.value - PAD.left - PAD.right));
const plotH = H - PAD.top - PAD.bottom;
const y = (v: number) => PAD.top + plotH - ((v - bottom.value) / range.value) * plotH;
const band = computed(() => plotW.value / Math.max(1, props.part.x.length));
const cx = (i: number) => PAD.left + band.value * (i + 0.5);
// Every k-th label of x, so labels never run into each other (about 56 px a label).
const every = computed(() => Math.max(1, Math.ceil((props.part.x.length * 56) / Math.max(1, plotW.value))));

const bars = computed(() => {
  const n = props.part.series.length;
  const w = Math.max(2, (band.value * 0.7) / n);
  return props.part.series.flatMap((s, si) =>
    s.values.map((v, i) => ({ key: `${si}-${i}`, si, x: cx(i) - (w * n) / 2 + w * si, y: Math.min(y(v), y(0)), w, h: Math.abs(y(v) - y(0)), title: `${props.part.x[i]} · ${s.name}: ${withUnit(v)}` })),
  );
});
const lines = computed(() =>
  props.part.series.map((s, si) => ({
    si,
    points: s.values.map((v, i) => `${cx(i)},${y(v)}`).join(' '),
    dots: s.values.map((v, i) => ({ key: i, x: cx(i), y: y(v), title: `${props.part.x[i]} · ${s.name}: ${withUnit(v)}` })),
  })),
);
const summary = computed(() => `${props.part.label}: a ${props.part.kind} chart of ${props.part.series.map((s) => s.name).join(' and ')} over ${props.part.x.length} labels, from ${props.part.x[0] ?? ''} to ${props.part.x.at(-1) ?? ''}.`);
</script>

<template>
  <figure class="min-w-0" :data-chart="part.id">
    <figcaption class="flex items-center gap-2">
      <span class="caps min-w-0 flex-1 truncate">{{ part.label }}<template v-if="part.unit"> · {{ part.unit }}</template></span>
      <button type="button" class="toggle" :aria-expanded="table" :aria-controls="`${id}-table`" @click="table = !table">{{ table ? 'Show the chart' : 'Show the numbers' }}</button>
    </figcaption>
    <div v-show="!table" ref="box" class="mt-1">
      <svg v-if="width" :width="width" :height="H" role="img" :aria-label="summary" class="block">
        <g class="grid">
          <template v-for="t in ticks" :key="t">
            <line :x1="PAD.left" :x2="width - PAD.right" :y1="y(t)" :y2="y(t)" />
            <text :x="PAD.left - 6" :y="y(t)" text-anchor="end" dominant-baseline="middle">{{ num(t) }}</text>
          </template>
          <template v-for="(label, i) in part.x" :key="i">
            <text v-if="i % every === 0" :x="cx(i)" :y="H - 6" text-anchor="middle">{{ label }}</text>
          </template>
        </g>
        <line v-if="crosses" class="zero" :x1="PAD.left" :x2="width - PAD.right" :y1="y(0)" :y2="y(0)" />
        <template v-if="part.kind === 'bar'">
          <rect v-for="b in bars" :key="b.key" :class="`s${b.si % 4}`" class="bar" :x="b.x" :y="b.y" :width="b.w - 1" :height="Math.max(0, b.h)" rx="2"><title>{{ b.title }}</title></rect>
        </template>
        <template v-else>
          <g v-for="l in lines" :key="l.si" :class="`s${l.si % 4}`">
            <polyline class="line" :points="l.points" :stroke-dasharray="DASH[l.si % 4]" />
            <circle v-for="d in l.dots" :key="d.key" class="dot" :cx="d.x" :cy="d.y" r="2.5"><title>{{ d.title }}</title></circle>
          </g>
        </template>
      </svg>
      <ul v-if="part.series.length > 1" class="mt-1 flex flex-wrap gap-x-4 gap-y-1" aria-hidden="true">
        <li v-for="(s, si) in part.series" :key="si" class="flex items-center gap-1.5 text-meta text-mut">
          <svg width="18" height="8" :class="`s${si % 4}`"><line v-if="part.kind === 'line'" class="line" x1="0" x2="18" y1="4" y2="4" :stroke-dasharray="DASH[si % 4]" /><rect v-else class="bar" x="5" y="0" width="8" height="8" rx="1" /></svg>{{ s.name }}
        </li>
      </ul>
    </div>
    <div v-if="table" :id="`${id}-table`" class="mt-1 max-h-[240px] overflow-auto">
      <table class="numbers">
        <thead><tr><th scope="col"><span class="sr-only">Label</span></th><th v-for="(s, si) in part.series" :key="si" scope="col">{{ s.name }}</th></tr></thead>
        <tbody>
          <tr v-for="(label, i) in part.x" :key="i"><th scope="row">{{ label }}</th><td v-for="(s, si) in part.series" :key="si">{{ s.values[i] === undefined ? '' : withUnit(s.values[i]) }}</td></tr>
        </tbody>
      </table>
    </div>
  </figure>
</template>

<style scoped>
.toggle {
  flex: none;
  min-height: 24px;
  padding: 0 4px;
  border-radius: 4px;
  color: var(--blue-text);
  font-size: var(--size-meta);
}
.toggle:hover {
  text-decoration: underline;
}
.grid line {
  stroke: var(--line);
}
.zero {
  stroke: var(--mut);
}
.grid text {
  fill: var(--mut);
  font-size: 11px;
}
/* The series, by a scale of data hues of their own: the first four, then again (and told apart by their dash). */
.s0 {
  --series: var(--blue);
}
.s1 {
  --series: var(--sand);
}
.s2 {
  --series: var(--cyan);
}
.s3 {
  --series: var(--pink);
}
.bar {
  fill: var(--series);
}
.line {
  fill: none;
  stroke: var(--series);
  stroke-width: 2;
}
.dot {
  fill: var(--series);
}
.numbers {
  width: 100%;
  font-size: var(--size-meta);
  font-variant-numeric: tabular-nums;
}
.numbers th,
.numbers td {
  padding: 3px 8px;
  border-bottom: 1px solid var(--line);
  text-align: right;
}
.numbers th[scope='row'],
.numbers thead th:first-child {
  color: var(--text2);
  font-weight: 400;
  text-align: left;
}
.numbers thead th {
  color: var(--mut);
  font-weight: 500;
}
.numbers td {
  color: var(--text);
}
</style>
