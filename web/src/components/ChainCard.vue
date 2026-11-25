<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import { projectOf, standingOf } from '../catalog';
import { defaultTab } from '../nav';
import { opened } from '../openChain';
import { waitTarget } from '../waits';
import { clockAt, plural } from '../ui';
import type { Chain } from '../core/generated/types.generated';
import Ago from './Ago.vue';
import ChainStandingBadge from './ChainStandingBadge.vue';
import Elapsed from './Elapsed.vue';
import Icon from './Icon.vue';
import ItemCard from './ItemCard.vue';
import Tip from './Tip.vue';
import { Badge } from '@/components/ui/badge';

// A chain on the board, the card every board's items wear (ItemCard): its title across the card, and under it its badge (where
// it stands, in one word never cut, the ring while it opens), what the server says beside the state and how long it has waited
// for the owner, has run, or when it finished. A chain that needs the owner names its first wait (ADR-0078): the badge is the
// wait's kind in the server's word (Key, Sign in, Approve, Failed), the line names who asks and what it asks, and how many more of his waits
// it holds. A chain that finished
// since the owner last viewed it says "new". A chain on a project names it (ChainStanding.project, the server's; ADR-0108). Why
// it stands there and how its result was judged are read in the chain, which a click opens at its first wait or on Overview; the search
// and the project the board is narrowed to go along. `finished`: in a column of finished chains, the time says since
// when. `on`: the chain open beside the column, marked by its surface alone.
const props = defineProps<{ chain: Chain; finished: boolean; on: boolean }>();
const route = useRoute();
const st = computed(() => standingOf(props.chain.id));
const wait = computed(() => st.value.wait);
const to = computed(() => {
  const target = wait.value?.chainId === props.chain.id ? waitTarget(wait.value) : undefined;
  const query = { q: route.query.q, project: route.query.project, col: route.query.col };
  if (target && typeof target !== 'string') return { ...target, query: { ...query, ...('query' in target ? target.query : {}) } };
  return { name: 'chain', params: { id: props.chain.id, tab: defaultTab }, query };
});
const project = computed(() => projectOf(st.value.project));
const loading = computed(() => opened.opening === props.chain.id && opened.view?.chain.chain.id !== props.chain.id);

// The server's words: the state first, then what it says beside it (Not done · not proven: the live effect): the state is the
// badge, the rest follows it on the line, where it wraps to two lines (what a chain Not done did not prove can run long): the
// hint says a longer one whole under the pointer, and the card shows it whole while it holds the keyboard's focus. A chain
// that waits says its wait instead.
const words = computed(() => st.value.text.split(' · '));
const state = computed(() => wait.value?.label ?? words.value[0]);
const rest = computed(() => wait.value?.ask ?? words.value.slice(1).join(' · '));
const asker = computed(() => wait.value?.who && !rest.value.startsWith(wait.value.who) ? wait.value.who : '');
const more = computed(() => Math.max(0, st.value.waits - 1));
</script>

<template>
  <ItemCard :to="to" :on="on" :title="chain.title" :data-chain="chain.id" :data-wait="wait?.kind">
    <template #badge><ChainStandingBadge :standing="st" :opening="loading">{{ state }}</ChainStandingBadge></template>
    <template #meta>
      <Badge v-if="finished && st.unseen" variant="info" size="count" data-unseen>new</Badge>
      <span v-if="project" class="inline-flex max-w-[16ch] min-w-0 items-center gap-1 text-text2" :data-project="project.id"><Icon class="flex-none" name="project" :size="11" /><span class="sr-only">Project</span><Tip :text="project.name" when-clipped><span class="truncate">{{ project.name }}</span></Tip></span>
      <Tip v-if="rest" :text="rest" when-clipped><span class="rest min-w-0 [overflow-wrap:anywhere]" data-rest><b v-if="asker" class="font-semibold text-text2">{{ asker }}:</b> {{ rest }}</span></Tip>
      <span v-if="more" class="whitespace-nowrap" data-more>+{{ plural(more, 'more wait', 'more waits') }}</span>
      <span v-if="!finished && st.checkBackAt" class="whitespace-nowrap" data-back>back at {{ clockAt(st.checkBackAt) }}</span>
      <Tip v-if="finished" text="when it finished"><span class="when ml-auto whitespace-nowrap"><Ago :at="chain.updatedAt" /></span></Tip>
      <Tip v-else-if="wait" text="how long it has waited for you"><span class="when ml-auto whitespace-nowrap" data-waited>waiting <Elapsed :from="wait.since" /></span></Tip>
      <Tip v-else text="elapsed time"><span class="when ml-auto whitespace-nowrap"><Elapsed :from="chain.createdAt" :to="chain.status === 'running' ? undefined : chain.updatedAt" /></span></Tip>
    </template>
  </ItemCard>
</template>

<style scoped>
.rest {
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  overflow: hidden;
}
[data-chain]:has(:focus-visible) .rest {
  -webkit-line-clamp: unset;
}
</style>
