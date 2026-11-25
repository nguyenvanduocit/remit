<script setup lang="ts">
// The dashed bar that shows more of a long list (older events, more chains, a run of steps it stands for). The parent
// places it.
defineEmits<{ click: [] }>();
</script>

<template>
  <button type="button" class="more" @click="$emit('click')"><slot /></button>
</template>

<style scoped>
.more {
  display: block;
  width: 100%;
  padding: 7px;
  border: 1px dashed var(--line2);
  border-radius: 8px;
  background: none;
  color: var(--mut);
  font-size: 12px;
  cursor: pointer;
}
.more:hover {
  border-color: var(--mut);
  color: var(--text);
}
</style>
