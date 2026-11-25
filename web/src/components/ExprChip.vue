<script setup lang="ts">
import Tip from './Tip.vue';

// An expression kept as it was written, because it is too involved to draw as parts: inline code, the same wherever a rule
// shows one. It stays whole on a line of its own when it fits, and breaks only when it is wider than the line.
defineProps<{ text: string }>();
</script>

<template>
  <Tip text="Kept as written: too involved to draw"><code class="expr code-chip text-text2" data-expr>{{ text }}</code></Tip>
</template>

<style scoped>
.expr {
  display: inline-block;
  max-width: 100%;
  vertical-align: bottom;
  font-variant-ligatures: none;
  overflow-wrap: anywhere;
}
</style>
