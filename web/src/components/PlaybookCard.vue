<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import type { Playbook } from '../core/generated/types.generated';
import { plural } from '../ui';
import CrewCard from './CrewCard.vue';
import { Badge } from '@/components/ui/badge';

// One playbook on the Playbooks board (CrewCard): its name, what it is for in a line, and at the end whether the owner changed it, else how many cards it starts with. A click
// opens its page; the search goes along.
const props = defineProps<{ playbook: Playbook; on: boolean }>();
const route = useRoute();
const to = computed(() => ({ name: 'playbooks', params: { playbookId: props.playbook.id }, query: { q: route.query.q } }));
</script>

<template>
  <CrewCard :to="to" :on="on" :title="playbook.name" :data-playbook="playbook.id">
    <template #badge>
      <Badge v-if="playbook.edited" variant="info">edited</Badge>
      <Badge v-else variant="quiet">{{ plural(playbook.seeds.length, 'card') }}</Badge>
    </template>
    <template #meta>{{ playbook.description }}</template>
  </CrewCard>
</template>
