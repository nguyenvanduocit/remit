<script setup lang="ts">
import { computed } from 'vue';
import { listedChain } from '../catalog';
import type { FiringLine } from '../core/generated/types.generated';

// What a firing acted on, wherever a firing is listed: the chain, by its title, a link that opens it on the agent the firing
// hired there; a chain no longer listed by its id, said to be deleted. A firing that acted on nothing shows nothing.
const props = defineProps<{ firing: FiringLine }>();
const chain = computed(() => (props.firing.chainId ? listedChain(props.firing.chainId) : undefined));
const to = computed(() => ({ name: 'chain', params: { id: props.firing.chainId }, query: props.firing.cardId ? { card: props.firing.cardId } : {} }));
</script>

<template>
  <RouterLink v-if="chain" class="min-w-0 text-blue-text hover:underline [overflow-wrap:anywhere]" :to="to" :aria-label="firing.cardId ? `Open the agent it hired in ${chain.title}` : `Open ${chain.title}`" data-acted-on>{{ chain.title }}</RouterLink>
  <span v-else-if="firing.chainId" class="min-w-0 text-mut [overflow-wrap:anywhere]" data-acted-on>{{ firing.chainId }} (since deleted)</span>
</template>
