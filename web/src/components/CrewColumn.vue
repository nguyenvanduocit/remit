<script setup lang="ts" generic="T">
import type { CrewColumn } from '../crewColumns';
import EmptyState from './EmptyState.vue';
import Icon from './Icon.vue';
import ShowMore from './ShowMore.vue';
import { Badge } from '@/components/ui/badge';

// One column of a Crew board: its group with the icon of it and how many it holds, then its rows in the server's order, a
// card each (the page's `row` slot draws it, told whether it is the one open), a page at a time (more shows the next). A
// Crew board has two or three columns, so each is wide: its cards stand side by side in it, as many abreast as have a
// readable width (a column narrower than that holds one card as wide as itself, never wider, so no badge is cut), and in one
// file beside an open item. The board shows its columns side by side; the open item's page shows
// one beside it (openId), marked, and adds a way to the other columns beside the count (#nav).
defineProps<{ column: CrewColumn<T>; keyOf: (row: T) => string; openId?: string }>();
defineEmits<{ more: [] }>();
defineSlots<{ row(props: { row: T; on: boolean }): unknown; nav(): unknown }>();
</script>

<template>
  <section class="flex min-h-0 flex-col" :aria-label="column.label" :data-column="column.key">
    <h2 class="caps flex items-center gap-[7px] px-1.5 pt-1 pb-2.5">
      <Icon class="text-blue-mark" :name="column.icon" :size="12" />{{ column.label }} <Badge variant="quiet" size="count" class="ml-auto tracking-normal" data-count>{{ column.count }}</Badge><slot name="nav" />
    </h2>
    <div class="grid flex-1 grid-cols-[repeat(auto-fill,minmax(min(250px,100%),1fr))] content-start gap-2 overflow-auto" data-rows>
      <template v-for="r in column.rows" :key="keyOf(r)"><slot name="row" :row="r" :on="openId === keyOf(r)" /></template>
      <ShowMore v-if="column.hidden > 0" class="col-span-full" @click="$emit('more')">Show {{ Math.min(30, column.hidden) }} more of {{ column.hidden }}</ShowMore>
      <!-- Centred in the column, under its name: the column's own width is where the eye looks for its cards. -->
      <EmptyState v-if="!column.count" class="col-span-full py-4 text-center text-xs" data-column-empty>{{ column.empty }}</EmptyState>
    </div>
  </section>
</template>
