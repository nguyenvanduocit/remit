<script setup lang="ts">
import { computed, ref } from 'vue';
import Ago from './Ago.vue';
import EmptyState from './EmptyState.vue';
import Icon from './Icon.vue';
import Markdown from './Markdown.vue';
import PageSection from './PageSection.vue';
import ShowMore from './ShowMore.vue';
import { useAct } from '../act';
import { catalog, listedChain } from '../catalog';
import { clockAt, STATUS_LABEL } from '../ui';
import type { Decision, OutcomeAnalyst } from '../core/generated/types.generated';
import { Button } from '@/components/ui/button';

// What the analyst has told the owner, newest first, a card each: its headline, the evidence under it, what it says to
// change as a block of its own, and the chains it found it in, each a link to that chain's Verification. A page at a time:
// an analyst that has run every day for a year has hundreds. And the analyst itself in the section's heading: where it
// stands, a run on request, and its chain, to talk to its Chair. The analyst is a chain like any other (the one with the
// outcomes capability), so steering it is talking to it.
defineProps<{ analyst: OutcomeAnalyst | null }>();
const act = useAct();
const PAGE = 10;
const shown = ref(1);
const insights = computed(() => catalog.insights.slice(0, shown.value));

// The analyst writes its body as Markdown and ends it with what to change under that heading, in bold: the block after
// the heading stands apart. Without one, the whole body is the evidence.
const CHANGE = /\*\*What to change:?\*\*:?\s*/i;
const partsOf = (d: Decision) => {
  const i = (d.why ?? '').search(CHANGE);
  if (i < 0) return { evidence: d.why ?? '', change: '' };
  return { evidence: d.why!.slice(0, i).trim(), change: d.why!.slice(i).replace(CHANGE, '').trim() };
};
// The chains an insight names, in the order it names them, each once.
const evidence = computed(() => insights.value.map((decision) => {
  const chains = new Set(`${decision.text}\n${decision.why ?? ''}`.match(/\bch_[a-z0-9]+\b/g) ?? []);
  return { decision, ...partsOf(decision), chains: [...chains], feedback: catalog.feedback.filter((item) => chains.has(item.home)) };
}));
</script>

<template>
  <PageSection title="Insights" :count="catalog.insights.length || undefined" id="insights" data-insights>
    <template #aside>
      <div class="flex min-w-0 flex-wrap items-center justify-end gap-2">
        <span v-if="analyst" class="min-w-0 truncate" data-analyst>
          Analyst {{ (STATUS_LABEL[analyst.status] ?? analyst.status).toLowerCase() }}<template v-if="analyst.lastRunAt"> · last run <Ago :at="analyst.lastRunAt" /></template><template v-if="analyst.nextRunAt"> · next check-back {{ clockAt(analyst.nextRunAt) }}</template>
        </span>
        <template v-if="analyst && analyst.status !== 'done'">
          <Button variant="outline" size="sm" @click="act('analystRun', [], 'The analyst is asked to run now')"><Icon name="play" :size="12" /> Analyze now</Button>
          <Button variant="outline" size="sm" as-child><RouterLink :to="{ name: 'chain', params: { id: analyst.chainId } }">Open the analyst chain</RouterLink></Button>
        </template>
        <Button v-else-if="catalog.insights.length" size="sm" @click="act('analystRun', [], 'Analyst started')">Start analyst</Button>
      </div>
    </template>
    <ol v-if="catalog.insights.length" class="m-0 grid list-none gap-3 p-0" :class="{ '@[1100px]:grid-cols-2': evidence.length > 1 }">
      <li v-for="{ decision: d, evidence: explanation, change, chains, feedback } in evidence" :key="d.id" class="insight" data-insight>
        <div class="flex items-start gap-2.5">
          <span class="flex h-6 w-6 flex-none items-center justify-center rounded-md bg-violet-bg text-violet-text" role="img" aria-label="Insight"><Icon name="spark" :size="13" /></span>
          <h3 class="min-w-0 flex-1 text-[13px] leading-[1.4] font-semibold [overflow-wrap:anywhere]" data-headline>{{ d.text }}</h3>
          <span class="flex-none text-[11.5px] text-mut"><Ago :at="d.createdAt" /></span>
        </div>
        <details v-if="explanation || change" class="mt-2.5">
          <summary class="w-fit cursor-pointer text-xs font-medium text-blue-text">Evidence and recommendation</summary>
          <Markdown v-if="explanation" class="mt-2" type="quiet" chain-tab="verification" :text="explanation" data-evidence />
          <div v-if="change" class="change mt-2.5" data-change>
            <div class="flex items-center gap-1.5 text-[11.5px] font-semibold text-sand"><Icon name="pencil" :size="12" />What to change</div>
            <Markdown class="mt-1" type="quiet" chain-tab="verification" :text="change" />
          </div>
        </details>
        <div v-if="chains.length" class="mt-2.5 flex flex-wrap items-center gap-1.5 text-[11.5px] text-mut" data-chains>
          <Icon name="link" :size="11" /><span>Found in</span>
          <RouterLink v-for="id in chains" :key="id" class="rounded bg-panel3 px-1.5 text-blue-text hover:underline [overflow-wrap:anywhere]" :to="{ name: 'chain', params: { id, tab: 'verification' } }" :data-chain="id">{{ listedChain(id)?.title ?? id }} · Verification</RouterLink>
        </div>
        <div v-if="feedback.length" class="mt-2 flex flex-wrap items-center gap-1.5 text-[11.5px] text-mut" data-related-feedback>
          <span>Feedback from these chains</span>
          <RouterLink v-for="item in feedback" :key="item.id" class="rounded bg-panel3 px-1.5 text-blue-text hover:underline [overflow-wrap:anywhere]" :to="{ name: 'feedback', params: { id: item.id } }">{{ item.name }} · {{ item.kind }}</RouterLink>
        </div>
      </li>
    </ol>
    <ShowMore v-if="catalog.insights.length > shown" class="mt-3" @click="shown += PAGE">Show {{ Math.min(PAGE, catalog.insights.length - shown) }} more of {{ catalog.insights.length - shown }}</ShowMore>
    <div v-if="!catalog.insights.length" class="flex flex-col items-start gap-3">
      <EmptyState class="text-[13px]">No insights yet. The analyst reads these figures and the chains behind them every day, and tells you here what to change, each with the chains it found it in.</EmptyState>
      <Button v-if="!analyst || analyst.status === 'done'" size="sm" @click="act('analystRun', [], 'Analyst started')">Start analyst</Button>
    </div>
  </PageSection>
</template>

<style scoped>
/* One insight: a card of its own in the section, its "what to change" a block on the sand of a note. */
.insight {
  padding: 12px 14px;
  border-radius: 8px;
  background: var(--bg2);
  box-shadow: var(--shadow-card);
}
.change {
  padding: 8px 10px;
  border: 1px solid var(--sand-line);
  border-radius: 8px;
  background: var(--sand-bg);
}
</style>
