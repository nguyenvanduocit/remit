<script setup lang="ts">
import { useRoute } from 'vue-router';
import type { ShownFeedbackColumn } from '../feedbackColumns';
import EmptyState from './EmptyState.vue';
import FeedbackCard from './FeedbackCard.vue';
import Icon from './Icon.vue';
import ShowMore from './ShowMore.vue';
import { Badge } from '@/components/ui/badge';

// One column of the Feedback board: its kind with the icon of it and how many it holds (the cards say which wait to be read), then its
// feedback newest first, a card each (and, while it holds none, the server's words for what lands there), a page at a time (more shows the next). The board shows its columns side by side; the open
// one's page shows one beside it (openId), marked, and adds a way to the other columns beside the count (#nav). A card opens its
// feedback and keeps the search; the column picked is dropped, since the feedback's own column is shown.
const ICON_TONE: Record<ShownFeedbackColumn['key'], string> = { practice: 'text-blue-text', difficulty: 'text-amber', complaint: 'text-red-text', suggestion: 'text-green-text' };
defineProps<{ column: ShownFeedbackColumn; openId?: string }>();
defineEmits<{ more: [] }>();
const route = useRoute();
const to = (id: string) => ({ name: 'feedback', params: { id }, query: { q: route.query.q } });
</script>

<template>
  <section class="flex min-h-0 flex-col" :aria-label="column.label" :data-column="column.key">
    <h2 class="caps flex items-center gap-[7px] px-1.5 pt-1 pb-2.5">
      <Icon :class="ICON_TONE[column.key]" :name="column.icon" :size="12" />{{ column.label }}
      <Badge variant="quiet" size="count" class="ml-auto tracking-normal" data-count>{{ column.count }}</Badge>
      <slot name="nav" />
    </h2>
    <div class="flex flex-1 flex-col gap-2 overflow-auto" data-rows>
      <FeedbackCard v-for="f in column.rows" :key="f.id" :feedback="f" :on="openId === f.id" :to="to(f.id)" />
      <ShowMore v-if="column.hidden > 0" @click="$emit('more')">Show {{ Math.min(30, column.hidden) }} more of {{ column.hidden }}</ShowMore>
      <!-- Centred in the column, under its name: the column's own width is where the eye looks for its cards. -->
      <EmptyState v-if="!column.count" class="py-4 text-center text-xs" data-column-empty>{{ column.empty }}</EmptyState>
    </div>
  </section>
</template>
