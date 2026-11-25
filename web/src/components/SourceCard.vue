<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import { sourceStanding } from '../crewColumns';
import type { ToolSource } from '../core/generated/types.generated';
import { plural } from '../ui';
import CrewCard from './CrewCard.vue';
import StandingBadge from './StandingBadge.vue';

// One tool source on the Tools board (CrewCard): its name, its standing (connected in green with a check, not connected in the
// owner's orange with a key: a value of its waits on him), and how many tools and connections it has. A click opens its page;
// the search goes along.
const props = defineProps<{ source: ToolSource; on: boolean }>();
const route = useRoute();
const to = computed(() => ({ name: 'tools', params: { sourceId: props.source.id }, query: { q: route.query.q } }));
const standing = computed(() => sourceStanding(props.source));
</script>

<template>
  <CrewCard :to="to" :on="on" :title="source.label" :data-source="source.id">
    <template #badge><StandingBadge :variant="standing.variant" :icon="standing.icon" data-standing>{{ standing.text }}</StandingBadge></template>
    <template #meta>{{ plural(source.tools.length, 'tool') }} · {{ plural(source.connections.length, 'connection') }}</template>
  </CrewCard>
</template>
