<script setup lang="ts">
import { ref } from 'vue';
import { useAnimating } from '../useAnimating';

// A status dot in the colour of the text around it. While `live` (an agent busy, chains running) a ring pings out of it:
// scaled and faded, on the compositor. Pauses off screen and stops under prefers-reduced-motion.
defineProps<{ live?: boolean }>();
const el = ref<HTMLElement>();
const play = useAnimating(el);
</script>

<template>
  <span ref="el" class="dot" aria-hidden="true"><i v-if="live" :style="{ animationPlayState: play }"></i></span>
</template>

<style scoped>
.dot {
  position: relative;
  display: inline-block;
  flex: none;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: currentColor;
}
.dot > i {
  position: absolute;
  inset: 0;
  border-radius: 50%;
  background: currentColor;
  animation: ping 1.8s ease-out infinite;
}
@keyframes ping {
  from {
    transform: scale(1);
    opacity: 0.55;
  }
  70%,
  to {
    transform: scale(3);
    opacity: 0;
  }
}
</style>
