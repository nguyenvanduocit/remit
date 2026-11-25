<script setup lang="ts">
import { ref } from 'vue';
import { useAnimating } from '../useAnimating';

// Something being worked on right now: a ring turning, in the colour of the text around it. It turns by transform alone (on
// the compositor), pauses off screen and stands still under prefers-reduced-motion.
withDefaults(defineProps<{ size?: number }>(), { size: 9 });
const el = ref<HTMLElement>();
const play = useAnimating(el);
</script>

<template>
  <i ref="el" class="spinner" :style="{ width: size + 'px', height: size + 'px', animationPlayState: play }" aria-hidden="true"></i>
</template>

<style scoped>
.spinner {
  display: inline-block;
  flex: none;
  border: 1.5px solid currentColor;
  border-right-color: transparent;
  border-radius: 50%;
  animation: turn 0.8s linear infinite;
}
@keyframes turn {
  to {
    transform: rotate(360deg);
  }
}
</style>
