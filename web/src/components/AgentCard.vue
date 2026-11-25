<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import type { Agent } from '../core/generated/types.generated';
import { modelEffort } from '../ui';
import AgentBadge from './AgentBadge.vue';
import CrewCard from './CrewCard.vue';
import { Badge } from '@/components/ui/badge';

// One agent on the Agents board (CrewCard): its lettered square (AgentBadge, the mark it wears everywhere), its name, what it
// is for in a line, and at the end the gate it signs, else whether the owner changed it, else the model it runs on. A click
// opens its page; the search goes along.
const props = defineProps<{ agent: Agent; on: boolean }>();
const route = useRoute();
const to = computed(() => ({ name: 'agents', params: { agentId: props.agent.id }, query: { q: route.query.q } }));
</script>

<template>
  <CrewCard :to="to" :on="on" :title="agent.name" :data-agent="agent.id">
    <template #chip><AgentBadge :agent="agent.id" :size="20" :label="`${agent.name}, agent`" /></template>
    <template #badge>
      <Badge v-if="agent.signs" variant="gate">signs {{ agent.signs }}</Badge>
      <Badge v-else-if="agent.edited" variant="info">edited</Badge>
      <Badge v-else variant="quiet">{{ modelEffort(agent.model || 'worker model', agent.effort) }}</Badge>
    </template>
    <template #meta>{{ agent.description }}</template>
  </CrewCard>
</template>
