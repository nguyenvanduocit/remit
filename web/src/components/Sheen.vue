<script setup lang="ts">
import { ref } from 'vue';
import { useAnimating } from '../useAnimating';

// Text that is live and still changing (a turn count climbing, tools in use): while `on`, a light sweeps across it. The light is a
// copy of the text in a masked window: the window slides one way and the copy inside it slides back by as much, so the copy
// stays over the text while the band of light crosses it. Both move by transform alone, so the text is never repainted.
// Pauses off screen; plain text under prefers-reduced-motion.
defineProps<{ text: string; on?: boolean }>();
const el = ref<HTMLElement>();
const play = useAnimating(el);
</script>

<template>
  <span ref="el" class="sheen" :style="{ '--play': play }">{{ text }}<span v-if="on" class="light" aria-hidden="true"><span>{{ text }}</span></span></span>
</template>

<style scoped>
.sheen {
  position: relative;
  display: inline-block;
}
.light {
  position: absolute;
  inset: 0;
  pointer-events: none;
  mask-image: linear-gradient(90deg, transparent 35%, black 50%, transparent 65%);
  animation: across 1.6s linear infinite var(--play);
}
.light > span {
  position: absolute;
  inset: 0;
  white-space: nowrap;
  color: var(--blue-hi);
  animation: back 1.6s linear infinite var(--play);
}
@keyframes across {
  from {
    transform: translateX(-100%);
  }
  to {
    transform: translateX(100%);
  }
}
@keyframes back {
  from {
    transform: translateX(100%);
  }
  to {
    transform: translateX(-100%);
  }
}
@media (prefers-reduced-motion: reduce) {
  .light {
    display: none;
  }
}
</style>
