<script setup lang="ts" generic="T">
import { computed, ref } from 'vue';
import ContextFold from './ContextFold.vue';
import EmptyState from './EmptyState.vue';
import SearchBox from './SearchBox.vue';

// One list of what an agent's context holds (its skills, tools, memory files), folded under its name and count: a
// session can carry 160 skills. A long list gets a search box. `grid` lays short names out in columns, a long name
// wrapping under itself rather than cut, so every name reads whole; the default slot draws a richer item.
const props = defineProps<{ title: string; items: T[]; text: (item: T) => string; open?: boolean; grid?: boolean; empty?: string }>();
const SEARCH_FROM = 10;
const query = ref('');
const shown = computed(() => {
  const q = query.value.trim().toLowerCase();
  return q ? props.items.filter((i) => props.text(i).toLowerCase().includes(q)) : props.items;
});
</script>

<template>
  <ContextFold :title="title" :count="items.length" :open="open">
    <template v-if="$slots.note" #note><slot name="note" /></template>
    <SearchBox v-if="items.length > SEARCH_FROM" v-model="query" class="mb-2" :placeholder="`Search ${items.length} ${title.toLowerCase()}`" :label="`Search ${title.toLowerCase()}`" />
    <ul v-if="shown.length" class="body" :class="{ cols: grid }">
      <li v-for="(item, i) in shown" :key="i" class="min-w-0"><slot :item="item">{{ text(item) }}</slot></li>
    </ul>
    <EmptyState v-else class="text-[12px]">{{ items.length ? `No ${title.toLowerCase()} match “${query}”.` : (empty ?? 'None.') }}</EmptyState>
  </ContextFold>
</template>

<style scoped>
.body {
  margin: 0;
  padding: 0;
  list-style: none;
  font-size: 12px;
  color: var(--text2);
}
.body > li {
  padding: 2px 0;
  overflow-wrap: anywhere;
}
.body.cols {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
  gap: 0 14px;
}
.body.cols > li {
  padding-left: 10px;
  text-indent: -10px;
}
</style>
