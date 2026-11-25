<script setup lang="ts">
// Facts as a two-column list: each <dt> names one, the <dd> after it says it (a chain's kind and directory, an agent's
// model and session). A value wraps between its words, and inside one only when that word alone is wider than the line. The
// names keep one width, wider only for a long one (a posted header's name), up to two fifths of the line. The rows may come
// from a component of their own (a fragment of <dt>/<dd>), so the styles reach into the list (`:deep`), not only its direct slot.
</script>

<template>
  <dl class="kv"><slot /></dl>
</template>

<style scoped>
.kv {
  display: grid;
  grid-template-columns: fit-content(40%) 1fr;
  gap: 6px 10px;
  margin: 0;
  font-size: 13px;
}
.kv :deep(dt) {
  min-width: 118px;
  color: var(--mut);
  overflow-wrap: anywhere;
}
.kv :deep(dd) {
  margin: 0;
  color: var(--text2);
  overflow-wrap: anywhere;
}
</style>
