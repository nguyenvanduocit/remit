<script setup lang="ts">
import { useRoute } from 'vue-router';
import type { ShownSubscriptionColumn } from '../subscriptionColumns';
import EmptyState from './EmptyState.vue';
import Icon from './Icon.vue';
import ShowMore from './ShowMore.vue';
import SubscriptionRow from './SubscriptionRow.vue';
import { Badge } from '@/components/ui/badge';

// One column of the Signals board as the server names it: its name with the icon of what happens there and how many it holds,
// then its subscriptions in the server's order (and, while it holds none, what puts one there), a card each, a page at a time (more shows the next). The board shows its columns side by side; the open
// subscription's page shows one beside it (openId), marked, and adds a way to the other columns beside the count (#nav). A card
// opens its line and keeps the search and the kind; the column picked is dropped, since the line's own column is shown.
const ICON_TONE: Record<ShownSubscriptionColumn['key'], string> = { attention: 'text-warning', coming: 'text-blue-mark', idle: 'text-mut' };
defineProps<{ column: ShownSubscriptionColumn; openId?: string }>();
defineEmits<{ more: [] }>();
const route = useRoute();
const to = (entry: string) => ({ name: 'subscriptions', params: { entry }, query: { q: route.query.q, kind: route.query.kind, hide: route.query.hide } });
</script>

<template>
  <section class="flex min-h-0 flex-col" :aria-label="column.label" :data-group="column.key">
    <h2 class="caps flex items-center gap-[7px] px-1.5 pt-1 pb-2.5">
      <Icon :class="ICON_TONE[column.key]" :name="column.icon" :size="12" />{{ column.label }} <Badge variant="quiet" size="count" class="ml-auto tracking-normal" data-count>{{ column.count }}</Badge><slot name="nav" />
    </h2>
    <div class="flex flex-1 flex-col gap-2 overflow-auto" data-rows>
      <SubscriptionRow v-for="l in column.lines" :key="l.id" :line="l" :on="openId === l.id" :to="to(l.id)" :data-entry="l.id" />
      <ShowMore v-if="column.hidden > 0" @click="$emit('more')">Show {{ Math.min(30, column.hidden) }} more of {{ column.hidden }}</ShowMore>
      <!-- Centred in the column, under its name: the column's own width is where the eye looks for its cards. -->
      <EmptyState v-if="!column.count" class="py-4 text-center text-xs" data-column-empty>{{ column.empty }}</EmptyState>
    </div>
  </section>
</template>
