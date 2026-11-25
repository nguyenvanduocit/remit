<script setup lang="ts">
import { useRoute } from 'vue-router';
import type { EventRecord } from '../core/generated/types.generated';
import EventCard from './EventCard.vue';
import EmptyState from './EmptyState.vue';
import Icon from './Icon.vue';
import ShowMore from './ShowMore.vue';
import { Badge } from '@/components/ui/badge';

// The Events list: every event newest first, a card each, a page at a time (more shows the next). The page shows it whole; the
// open event's page shows it beside it (openId), marked. A card opens its event and keeps the search.
defineProps<{ rows: readonly EventRecord[]; count: number; hidden: number; openId?: string }>();
defineEmits<{ more: [] }>();
const route = useRoute();
const to = (id: string) => ({ name: 'events', params: { id }, query: { q: route.query.q } });
</script>

<template>
  <section class="flex min-h-0 flex-col" aria-label="Events" data-list>
    <h2 class="caps flex items-center gap-[7px] px-1.5 pt-1 pb-2.5">
      <Icon name="inbox" :size="12" />Events <Badge variant="quiet" size="count" class="ml-auto tracking-normal" data-count>{{ count }}</Badge>
    </h2>
    <div class="flex flex-1 flex-col gap-2 overflow-auto" data-rows>
      <EventCard v-for="e in rows" :key="e.id" :event="e" :on="openId === e.id" :to="to(e.id)" />
      <ShowMore v-if="hidden > 0" @click="$emit('more')">Show {{ Math.min(30, hidden) }} more of {{ hidden }}</ShowMore>
      <EmptyState v-if="!count" class="py-4 text-center text-xs">Nothing here.</EmptyState>
    </div>
  </section>
</template>
