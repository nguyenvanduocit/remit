<script setup lang="ts">
// A part folded under its line, opened by click or keyboard: a part of an agent's context, a prompt or a thought in the
// feed. The line carries no marker: its pointer cursor, and its content appearing below on click, say it opens.
defineProps<{ open?: boolean }>();
</script>

<template>
  <details class="fold" :open="open">
    <summary class="flex min-h-6 cursor-pointer items-center gap-1.5"><slot name="summary" /></summary>
    <slot />
  </details>
</template>

<style scoped>
summary {
  list-style: none;
  border-radius: 4px;
}
summary::-webkit-details-marker {
  display: none;
}
</style>
