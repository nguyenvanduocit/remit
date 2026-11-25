<script setup lang="ts">
import type { RouteLocationRaw } from 'vue-router';
import ItemCard from './ItemCard.vue';
import Tip from './Tip.vue';

// The card every Crew board's items wear (a playbook, an agent, a skill, a tool source), on the board's card (ItemCard): the
// `title` cut at two lines (the hint says it whole; `mono` for a name the owner types), beside it the one `badge` that is
// never cut, and under them one line: the `chip` of what kind it is (a shape on a tint, named for a screen reader), then the
// `meta`, cut at its end. `on` marks the one open by its surface. The column it sits in spaces the cards.
defineProps<{ to: RouteLocationRaw; on: boolean; title: string; mono?: boolean }>();
</script>

<template>
  <ItemCard :to="to" :on="on">
    <span class="flex min-w-0 flex-1 flex-col gap-1">
      <span class="flex items-start gap-2">
        <Tip :text="title" when-clipped><span class="line-clamp-2 min-w-0 flex-1 leading-[1.35] font-semibold [overflow-wrap:anywhere]" :class="mono ? 'font-mono text-[12px]' : 'text-[12.5px]'" data-title>{{ title }}</span></Tip>
        <span class="mt-px flex flex-none" data-badge><slot name="badge" /></span>
      </span>
      <span class="meta flex min-w-0 items-center gap-1.5 text-[11.5px]"><slot name="chip" /><span class="truncate" data-meta><slot name="meta" /></span></span>
    </span>
  </ItemCard>
</template>

<style scoped>
.meta {
  color: var(--mut);
}
</style>
