<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { useQueryRef } from '../nav';
import { listedChain } from '../catalog';
import { queueKey, queueName } from '../landings';
import { openRepos, repos } from '../releases';
import type { Landing } from '../core/generated/types.generated';
import EmptyPage from './EmptyPage.vue';
import Icon from './Icon.vue';
import LandingCard from './LandingCard.vue';
import ListState from './ListState.vue';
import PageTabs from './PageTabs.vue';
import Tip from './Tip.vue';
import { Badge } from '@/components/ui/badge';

// Every merge queue Remit lands through (ADR-0114): a column per repository and base, its landings in the order the server
// lays them (being checked, then ready in the order the queue takes them, then what landed, went back or was taken back in
// the last day, newest first; a version still judged is not in it). Nothing here orders or decides. Each repository is read
// when the page shows and kept current by what the server pushes of it (RepoView). ?queue= (a landing's "Its queue") brings
// that queue's column into view.
openRepos(true);
onBeforeUnmount(() => openRepos(false));
// A column per queue, in the server's order of repositories and of each one's bases.
const queues = computed(() => (repos.read ? repos.views.flatMap((v) => v.queues.map((q) => ({ ...q, repo: v.repo }))) : null));

const wanted = useQueryRef<string>('queue', '');
const strip = ref<HTMLElement>();
watch([queues, wanted], () => nextTick(() => strip.value?.querySelector<HTMLElement>(`[data-queue="${CSS.escape(wanted.value)}"]`)?.scrollIntoView({ block: 'nearest', inline: 'nearest' })));

const chainOf = (l: Landing) => listedChain(l.chainId)?.title ?? l.chainId;
const toLanding = (l: Landing) => ({ name: 'chain', params: { id: l.chainId, tab: 'landings' }, query: { landing: l.id } });
</script>

<template>
  <section class="flex min-h-0 flex-1 flex-col overflow-hidden" aria-label="Merge queues">
    <PageTabs group="work">
    </PageTabs>
    <EmptyPage v-if="queues && !queues.length" title="No merge queue moving" icon="merge">
      <template #what>Each repository’s queue shows here while a change waits in it or landed in the last day: what is being checked, what is ready in the order Remit lands it, and what landed, went back to its Chair or was taken back.</template>
      <template #how>
        <li>A Chair lands its chain’s change once its work is done; its gates judge that version.</li>
        <li>Once every gate approves, the version joins its repository’s queue by itself: Remit combines it with the base, runs the checks and pushes it.</li>
        <li>A change that conflicts or fails the checks goes back to its Chair; one that breaks the base after it landed is taken back.</li>
      </template>
    </EmptyPage>
    <div v-else-if="!queues" class="px-pane pt-3.5"><ListState what="the merge queues" :held="false" /></div>
    <div v-if="queues?.length" ref="strip" class="strip grid min-h-0 flex-1 grid-flow-col gap-gutter overflow-x-auto px-pane pt-2.5 pb-3.5" data-strip>
      <section v-for="q in queues" :key="queueKey(q)" class="queue flex min-h-0 flex-col rounded-lg p-2" :class="{ wanted: queueKey(q) === wanted }" :aria-labelledby="`queue-${queueKey(q)}`" :data-queue="queueKey(q)">
        <div class="flex items-start gap-[7px] px-1.5 pt-1 pb-2.5">
          <Icon class="mt-[3px] flex-none text-blue-mark" name="merge" :size="12" />
          <h2 :id="`queue-${queueKey(q)}`" class="flex min-w-0 flex-1 flex-col">
            <Tip :text="queueName(q)" when-clipped><span class="mono truncate text-[12.5px] font-semibold text-text">{{ queueName(q) }}</span></Tip>
            <span v-if="q.landings[0]" class="truncate text-[11px] font-normal text-mut">{{ q.landings[0].host }}</span>
          </h2>
          <Badge variant="quiet" size="count" class="flex-none">{{ q.landings.length }}</Badge>
        </div>
        <RouterLink class="mx-1.5 mb-2 inline-flex items-center gap-1 self-start text-[12px] text-blue-text hover:underline" :to="{ name: 'releases', query: { repo: q.repo } }" data-releases-link>Its releases<Icon name="arrow-right" :size="11" /></RouterLink>
        <ul class="m-0 flex min-h-0 flex-1 list-none flex-col gap-2 overflow-auto p-0" :aria-label="`Landings in ${queueName(q)}`" data-rows>
          <li v-for="l in q.landings" :key="l.id"><LandingCard :landing="l" :where="chainOf(l)" :to="toLanding(l)" :on="false" /></li>
        </ul>
      </section>
    </div>
  </section>
</template>

<style scoped>
/* A column per queue, at least 300px and at most 420px wide, side by side; more than fit scroll sideways. */
.strip {
  grid-auto-columns: minmax(300px, 420px);
}
.queue {
  background: var(--bg2);
}
/* The queue a landing's "Its queue" opened on: its column is pressed into the page, as an open card is. */
.queue.wanted {
  background: var(--well);
  box-shadow: var(--shadow-pressed);
}
</style>
