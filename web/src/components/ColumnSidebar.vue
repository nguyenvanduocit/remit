<script setup lang="ts">
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import Sidebar from './Sidebar.vue';
import Tip from './Tip.vue';

// A board beside the item open on its page, one column at a time (useColumnSlide says which): the sidebar's head holds what
// the page puts there (a search) and the marks, the columns as tabs (←/→ slide along them), each saying which and how many;
// the column itself is the page's (the slot, keyed by the column so it slides out and the next in, from the side it lies on).
// `label` names the marks for a screen reader.
defineProps<{ columns: readonly { key: string; label: string; count: number }[]; shownKey: string; slide: 'next' | 'prev'; label: string }>();
defineEmits<{ go: [key: string] }>();
</script>

<template>
  <!-- The tabs hold the column's search and marks (the sidebar's head) and the column itself (its list), so they take the
       sidebar as their own element. -->
  <Tabs as-child class="gap-0" :model-value="shownKey" @update:model-value="$emit('go', String($event))">
    <Sidebar>
      <template #head>
        <slot name="head" />
        <TabsList variant="marks" class="mt-2" :aria-label="label">
          <Tip v-for="c in columns" :key="c.key" :text="`${c.label} · ${c.count}`">
            <TabsTrigger class="tab" :value="c.key" :aria-label="`${c.label}, ${c.count}`"><i class="mark block w-full rounded-[2px] bg-mut in-data-active:bg-blue"></i></TabsTrigger>
          </Tip>
        </TabsList>
      </template>
      <!-- Clipped sideways only, so the column sliding in draws no scrollbar. -->
      <TabsContent :value="shownKey" class="flex overflow-x-clip">
        <Transition :name="slide" mode="out-in"><slot /></Transition>
      </TabsContent>
    </Sidebar>
  </Tabs>
</template>

<style scoped>
/* Marks swell under the pointer like the macOS dock: the one hovered most, its neighbours less. A mark is as tall as the largest it
   gets (10px) and is scaled down from its middle, so the swell moves no layout: the row and everything below it stay where they are. */
.mark {
  --s: 0.3;
  height: 10px;
  transform: scaleY(var(--s));
  transition: transform var(--dur-fast) var(--ease-enter);
}
[data-active] > .mark {
  --s: 0.5;
}
.tab:hover > .mark {
  --s: 1;
}
.tab:hover + .tab > .mark,
.tab:has(+ .tab:hover) > .mark {
  --s: 0.7;
}
.tab:hover + .tab + .tab > .mark,
.tab:has(+ .tab + .tab:hover) > .mark {
  --s: 0.5;
}
.next-enter-active,
.prev-enter-active {
  transition:
    transform var(--dur-base) var(--ease-enter),
    opacity var(--dur-base) var(--ease-enter);
}
.next-leave-active,
.prev-leave-active {
  transition:
    transform var(--dur-fast) ease-in,
    opacity var(--dur-fast) ease-in;
}
.next-enter-from,
.prev-leave-to {
  transform: translateX(24px);
  opacity: 0;
}
.next-leave-to,
.prev-enter-from {
  transform: translateX(-24px);
  opacity: 0;
}
@media (prefers-reduced-motion: reduce) {
  .mark {
    transition: none;
  }
  .next-enter-from,
  .next-leave-to,
  .prev-enter-from,
  .prev-leave-to {
    transform: none;
  }
}
</style>
