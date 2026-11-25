<script setup lang="ts">
import { computed } from 'vue';
import { authoring } from '../playbook';
import { chainRecords } from '../catalog';
import AgentBadge from './AgentBadge.vue';
import StatusChip from './StatusChip.vue';
import Tip from './Tip.vue';

// The author chains still at work, under the page whose records it writes (playbooks, agents, skills): the
// owner told the Assistant what he wants, and here he watches it done. The list above changes as the author saves.
const working = computed(() => authoring(chainRecords.value));
</script>

<template>
  <div v-if="working.length" class="border-t border-line pb-2">
    <div v-for="c in working" :key="c.id" class="flex items-center gap-2 px-3 pt-2 text-[11.5px] text-mut">
      <AgentBadge agent="chair" :size="18" />
      <StatusChip :status="c.status" />
      <Tip :text="c.title" when-clipped><b class="min-w-0 truncate font-semibold text-text2">{{ c.title }}</b></Tip>
      <RouterLink class="flex-none text-blue-text hover:underline" :to="{ name: 'chain', params: { id: c.id } }">Open chain</RouterLink>
    </div>
  </div>
</template>
