<script setup lang="ts">
import { computed } from 'vue';
import type { Face, FaceAccessory, FaceShape } from '../core/generated/types.generated';

// An employee's face (ADR-0093), `size` px wide: the shape and hue of its lineage, so siblings read as one family, with an
// accessory of its own, on a 32-unit grid. The server derives the face; this only draws it. The hue is the server's, the
// lightness and chroma the theme's (--face-l, --face-c), so every hue keeps its contrast. `label` names it where nothing
// beside it does (the face alone, as a mark); without one it is decoration beside the employee's name.
const props = withDefaults(defineProps<{ face: Face; size?: number; label?: string }>(), { size: 20 });

const BODY: Record<FaceShape, { d: string; eyes: number }> = {
  circle: { d: 'M16 6a12 12 0 1 1 0 24a12 12 0 1 1 0-24Z', eyes: 17 },
  square: { d: 'M9 6h14a5 5 0 0 1 5 5v14a5 5 0 0 1-5 5H9a5 5 0 0 1-5-5V11a5 5 0 0 1 5-5Z', eyes: 17 },
  hexagon: { d: 'M16 5.5l10.8 6.25v12.5L16 30.5l-10.8-6.25v-12.5Z', eyes: 17.5 },
  triangle: { d: 'M16 5.5l13 24h-26Z', eyes: 22 },
  diamond: { d: 'M16 5l13 13l-13 13l-13-13Z', eyes: 18 },
  drop: { d: 'M16 5c3.5 4.5 11 11 11 17a11 11 0 0 1-22 0c0-6 7.5-12.5 11-17Z', eyes: 21 },
};

const body = computed(() => BODY[props.face.shape]);
const accessory = computed<FaceAccessory>(() => props.face.accessory);
const hue = computed(() => ({ '--hue': String(props.face.hue) }));
</script>

<template>
  <svg class="face" :width="size" :height="size" viewBox="0 0 32 32" :style="hue" :role="label ? 'img' : undefined" :aria-label="label" :aria-hidden="label ? undefined : 'true'" :data-shape="face.shape" :data-accessory="face.accessory">
    <path class="skin" :d="body.d" />
    <template v-if="accessory === 'visor'">
      <rect class="ink" x="8" :y="body.eyes - 2.6" width="16" height="5.2" rx="2.6" />
      <rect class="skin" x="11" :y="body.eyes - 0.6" width="10" height="1.2" rx="0.6" />
    </template>
    <template v-else>
      <circle class="ink" cx="12.5" :cy="body.eyes" r="1.9" />
      <circle class="ink" cx="19.5" :cy="body.eyes" r="1.9" />
    </template>
    <g v-if="accessory === 'antenna'"><path class="wire" d="M16 5.5V2.2" /><circle class="skin" cx="16" cy="2" r="1.8" /></g>
    <ellipse v-else-if="accessory === 'halo'" class="wire" cx="16" cy="2.4" rx="6.5" ry="1.6" />
    <path v-else-if="accessory === 'spark'" class="skin" d="M27 0.5l1.1 2.9l2.9 1.1l-2.9 1.1L27 8.5l-1.1-2.9l-2.9-1.1l2.9-1.1Z" />
    <path v-else-if="accessory === 'leaf'" class="skin" d="M16 5.5c0-3 2.5-5 6-5c0 3.2-2.4 5-6 5Z" />
    <path v-else-if="accessory === 'bow'" class="skin" d="M16 3.5l-5-3v6Zm0 0l5-3v6Z" />
  </svg>
</template>

<style scoped>
.face {
  display: block;
  flex: none;
  overflow: visible;
}
.skin {
  fill: oklch(var(--face-l) var(--face-c) var(--hue));
}
.ink {
  fill: var(--face-ink);
}
.wire {
  fill: none;
  stroke: oklch(var(--face-l) var(--face-c) var(--hue));
  stroke-width: 1.6;
  stroke-linecap: round;
}
</style>
