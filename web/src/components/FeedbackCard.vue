<script setup lang="ts">
import type { RouteLocationRaw } from 'vue-router';
import type { Feedback } from '../core/generated/types.generated';
import { short } from '../ui';
import Ago from './Ago.vue';
import ItemCard from './ItemCard.vue';
import { Badge } from '@/components/ui/badge';

// One piece of feedback on the Feedback board, the card every board's items wear (ItemCard): who sent it across the card, under
// it its first words as plain text (its page says them whole, as the agent wrote them, in Markdown) cut at two lines, and under
// those the badge of one not read yet and when. Its kind is its column's, so the card does not repeat it. One that waits to be
// read shows by its surface (the owner's colour, as what waits on him does everywhere) and its badge; the one open by its
// surface. The column spaces the cards.
defineProps<{ feedback: Feedback; to: RouteLocationRaw; on: boolean }>();
const PREVIEW = 240;
</script>

<template>
  <ItemCard :to="to" :on="on" :title="feedback.name" :class="{ unread: !feedback.readAt }" :data-feedback-id="feedback.id" :data-unread="feedback.readAt ? undefined : ''">
    <template #badge><Badge v-if="!feedback.readAt" variant="needs" data-unread-badge>New</Badge></template>
    <template #meta>
      <span class="order-first line-clamp-2 min-w-0 basis-full text-text2 [overflow-wrap:anywhere]" data-text>{{ short(feedback.text, PREVIEW) }}</span>
      <span class="ml-auto whitespace-nowrap"><Ago :at="feedback.createdAt" /></span>
    </template>
  </ItemCard>
</template>

<style scoped>
.unread:not(.on) {
  background: var(--orange-bg);
}
</style>
