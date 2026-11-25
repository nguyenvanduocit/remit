<script setup lang="ts">
import { computed } from 'vue';
import type { ChainView } from '../core/generated/types.generated';
import EmptyState from './EmptyState.vue';
import PageBody from './PageBody.vue';
import WorkflowRun from './WorkflowRun.vue';

// The chain's workflow runs, a tab of their own: one list of runs, newest first, each with its steps and the cards they put to
// work (WorkflowRun). Their cards stand in the agent tree too; this is where the script that drives them shows.
const props = defineProps<{ view: ChainView }>();
const runs = computed(() => [...props.view.workflows].reverse());
const cards = computed(() => new Map(props.view.cards.map((c) => [c.id, c])));
const byId = computed(() => new Map(props.view.workflows.map((w) => [w.id, w])));
</script>

<template>
  <PageBody data-workflows>
    <div v-if="runs.length" class="flex flex-col gap-5">
      <WorkflowRun v-for="w in runs" :key="w.id" :run="w" :cards="cards" :runs="byId" />
    </div>
    <EmptyState v-else>No workflow has run in this chain yet.</EmptyState>
  </PageBody>
</template>
