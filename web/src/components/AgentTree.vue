<script setup lang="ts">
import { computed, reactive } from 'vue';
import { outline, treeOf } from '../tree';
import type { ChainView } from '../core/generated/types.generated';
import AgentRow from './AgentRow.vue';
import EmptyState from './EmptyState.vue';
import Outline from './Outline.vue';
import PageBody from './PageBody.vue';

// Who works under whom, as an outline the server lays out: the Chair on top, its agents one level in, each agent of the
// team under the one whose work it builds on or judges, and a run of work handed on from one agent to the next as one
// straight column (outline). The cards Remit started (the judges) are not the Chair's agents: they stand in a list of
// their own below, with no line to it. Every line says status, current work, turns and time; a branch folds to its line;
// a click opens the agent.
const props = defineProps<{ view: ChainView }>();
const roots = computed(() => treeOf(props.view));
const folded = reactive(new Set<string>());
const toggle = (id: string) => (folded.has(id) ? folded.delete(id) : folded.add(id));
const rows = computed(() => (roots.value ? outline(roots.value, folded) : []));
const peers = computed(() => (roots.value?.peers ?? []).map((node) => ({ node, level: 0, parent: -1 })));
</script>

<template>
  <PageBody feed data-tree>
    <div v-if="roots">
      <Outline :rows="rows" label="The Chair and its team">
        <AgentRow v-for="r in rows" :key="r.node.card.id" :node="r.node" :level="r.level" :folded="folded.has(r.node.card.id)" :team="r.parent >= 0" @toggle="toggle(r.node.card.id)" />
      </Outline>
      <template v-if="peers.length">
        <h2 class="caps mt-5 mb-1.5 px-2">Started by Remit</h2>
        <Outline :rows="peers" label="Started by Remit">
          <AgentRow v-for="p in peers" :key="p.node.card.id" :node="p.node" :level="0" />
        </Outline>
      </template>
    </div>
    <EmptyState v-else>No agent has started yet.</EmptyState>
  </PageBody>
</template>
