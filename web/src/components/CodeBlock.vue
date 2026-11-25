<script setup lang="ts">
import CopyButton from './CopyButton.vue';

// A block of code or output, as it is: by default text to copy and paste, which scrolls inside its own box and never wraps,
// so a long line cannot widen the page; `wrap` for text read rather than pasted (a posted body, a brief), which wraps. The
// Copy button stands at the end of the box, inside it, so boxes one under another end at the same place; `actions` takes
// its place when the text is not what is copied or more than copying is offered (a key that is hidden until the owner asks).
defineProps<{ text: string; label: string; wrap?: boolean }>();
</script>

<template>
  <div class="code flex items-start gap-2">
    <pre class="m-0 min-w-0 flex-1 py-[3px]" :class="wrap ? 'whitespace-pre-wrap [overflow-wrap:anywhere]' : 'overflow-x-auto whitespace-pre'" data-code>{{ text }}</pre>
    <slot name="actions"><CopyButton :text="text" :label="label" /></slot>
  </div>
</template>

<style scoped>
.code {
  padding: 6px 6px 6px 10px;
  border: 1px solid var(--line);
  border-radius: 6px;
  background: var(--bg);
  font-family: var(--mono);
  font-size: 11.5px;
  line-height: 1.5;
  color: var(--text2);
}
</style>
