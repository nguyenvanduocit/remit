<script setup lang="ts">
import type { SubscriptionLine } from '../core/generated/types.generated';
import EmptyState from './EmptyState.vue';
import PageSection from './PageSection.vue';
import SubscriptionRow from './SubscriptionRow.vue';

// The subscriptions that listen to an open signal, the cards of the Subscriptions tab, each opening its page there.
defineProps<{ lines: SubscriptionLine[]; events?: boolean }>();
</script>

<template>
  <PageSection title="Subscriptions" :count="lines.length" id="listening" data-listening>
    <div class="flex flex-col gap-2">
      <SubscriptionRow v-for="l in lines" :key="l.id" :line="l" :on="false" :to="{ name: 'subscriptions', params: { entry: l.id } }" :data-entry="l.id" />
      <EmptyState v-if="!lines.length" class="text-xs" data-empty>No subscription listens to it<template v-if="events">, so what comes in is stored and nothing is done: tell the Assistant what should happen</template>.</EmptyState>
    </div>
  </PageSection>
</template>
