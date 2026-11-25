<script setup lang="ts">
import { computed } from 'vue';
import { chainRecords } from '../catalog';
import type { EventRecord } from '../core/generated/types.generated';
import EmptyState from './EmptyState.vue';
import FiringItem from './FiringItem.vue';
import PageBody from './PageBody.vue';
import PageSection from './PageSection.vue';

// What became of an event: the firing of each subscription that held for it, oldest first, in the server's words, the
// subscription linking to its page while it exists; and the chains the event started, each a link. An event no subscription held
// for says so and says how one is made: by telling the Assistant.
const props = defineProps<{ event: EventRecord }>();
const started = computed(() => chainRecords.value.filter((c) => c.eventId === props.event.id));
</script>

<template>
  <PageBody>
    <PageSection title="Firings" :count="event.firings.length" id="firings" data-firings>
      <EmptyState v-if="!event.firings.length" data-none>No subscription held for it, so it was stored and nothing was done. Tell the Assistant what should happen when an event like this comes in.</EmptyState>
      <ul v-else class="m-0 flex list-none flex-col p-0">
        <FiringItem v-for="(f, i) in event.firings" :key="i" :firing="f" class="border-b border-line py-2 last:border-b-0">
          <template #subscription>
            <RouterLink v-if="f.subscriptionId" class="hit w-fit font-semibold text-blue-text hover:underline [overflow-wrap:anywhere]" :to="{ name: 'subscriptions', params: { entry: f.subscriptionId } }" data-subscription-link>{{ f.title }}</RouterLink>
            <span v-else class="font-semibold text-mut [overflow-wrap:anywhere]" data-subscription-gone>{{ f.title }} (since deleted)</span>
          </template>
        </FiringItem>
      </ul>
    </PageSection>
    <PageSection v-if="started.length" title="Chains it started" :count="started.length" id="started" class="mt-4" data-started>
      <ul class="m-0 flex list-none flex-col gap-1 p-0">
        <li v-for="c in started" :key="c.id"><RouterLink class="hit text-blue-text hover:underline [overflow-wrap:anywhere]" :to="{ name: 'chain', params: { id: c.id } }" data-chain-link>{{ c.title }}</RouterLink></li>
      </ul>
    </PageSection>
  </PageBody>
</template>
