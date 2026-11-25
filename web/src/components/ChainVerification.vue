<script setup lang="ts">
import { computed, ref } from 'vue';
import { useRouter } from 'vue-router';
import EmptyState from './EmptyState.vue';
import PageSection from './PageSection.vue';
import ShowMore from './ShowMore.vue';
import StandingBadge from './StandingBadge.vue';
import StandingMark from './StandingMark.vue';
import Tip from './Tip.vue';
import VerdictEntry from './VerdictEntry.vue';
import { GATE_LOOK, plural } from '../ui';
import type { ChainView, GateState } from '../core/generated/types.generated';
import { Button } from '@/components/ui/button';

// One chain's output quality, as the server reads its verdicts (view.verification): a card per gate (the reviewer, the QC,
// the security reviewer when a change touches a rail, and the goal judge), each with the badge of its state, then who said
// what, a card per judge. To change what is checked or how strictly, the owner tells the Chair or the Assistant.
const props = defineProps<{ view: ChainView }>();
const router = useRouter();
// The judges a page at a time: a chain across many repositories has a reviewer and a QC per change.
const JUDGES = 24;
const judgesShown = ref(JUDGES);
const judges = computed(() => props.view.verification.judges.slice(0, judgesShown.value));
const watch = (cardId: string) => router.push({ name: 'chain', params: { id: props.view.chain.chain.id, tab: 'tree' }, query: { card: cardId } });
// The word each state's badge says, beside its icon and tone (GATE_LOOK).
const WORD: Record<GateState, string> = { approved: 'Passed', stale: 'Stale', changes: 'Changes asked', running: 'Checking', none: 'Not yet' };
</script>

<template>
  <!-- The tab's width is its pane's (the board's column and the drawer take the rest), not the window's, so the layout answers to
       the container: the gates sit three or four in a row once each has room, four going two by two first, else stacked
       (never an orphan), and the verdicts take one, two or three columns, three being as many as the gates. The goal judge is the one verdict on the whole chain,
       not on one change: the server lists it first and it takes the full row. -->
  <div class="@container flex flex-col gap-5">
    <PageSection title="Gates" :count="view.verification.gates.length" id="gates" data-gates>
      <template #aside>each change passes a reviewer and a QC, and a security reviewer when it touches a rail; the goal judge rules on the whole chain</template>
      <div class="grid gap-3" :class="view.verification.gates.length === 4 ? '@xl:grid-cols-2 @5xl:grid-cols-4' : '@xl:grid-cols-3'">
        <article v-for="g in view.verification.gates" :key="g.key" class="gate" :data-gate="g.key" :data-state="g.state" :aria-label="g.label">
          <div class="flex items-start gap-2">
            <h3 class="min-w-0 flex-1 text-[12.5px] font-medium text-text2">{{ g.label }}</h3>
            <StandingBadge :variant="GATE_LOOK[g.state].tone" :working="g.state === 'running'" :icon="GATE_LOOK[g.state].icon" class="flex-none" data-badge>{{ WORD[g.state] }}</StandingBadge>
          </div>
          <div class="mt-1 text-[15px] leading-snug font-semibold [overflow-wrap:anywhere]" :class="{ 'font-medium text-mut': g.state === 'none' }" data-line>{{ g.line }}</div>
          <Tip :text="g.who" when-clipped><div class="mt-0.5 min-h-4 truncate text-xs text-mut" data-who>{{ g.who }}</div></Tip>
          <Tip v-if="g.rounds.length" text="Every verdict this gate has given, oldest first: a check approved, a cross asked for changes">
            <div class="mt-1.5 flex flex-wrap items-center gap-1 text-xs text-mut" role="img" :aria-label="`${plural(g.rounds.length, 'verdict')}: ${g.rounds.map((ok) => (ok ? 'approved' : 'changes asked')).join(', ')}`">
              <StandingMark v-for="(ok, i) in g.rounds" :key="i" v-bind="GATE_LOOK[ok ? 'approved' : 'changes']" />
              <span class="ml-1">{{ plural(g.rounds.length, 'verdict') }}</span>
            </div>
          </Tip>
          <Button v-if="g.watch" variant="outline" size="sm" class="mt-2.5" @click="watch(g.watch)">Watch it work</Button>
        </article>
      </div>
    </PageSection>

    <PageSection title="Verdicts" :count="view.verification.judges.length || undefined" id="verdicts" data-verdicts>
      <template #aside>the goal check first, then each judge of a change, newest round in view</template>
      <div v-if="view.verification.judges.length" class="grid gap-3 @2xl:grid-cols-2 @6xl:grid-cols-3">
        <VerdictEntry v-for="j in judges" :key="j.key" :judge="j" :open-card="watch" :class="{ 'col-span-full': j.role === 'judge' }" />
      </div>
      <ShowMore v-if="view.verification.judges.length > judgesShown" class="mt-3" @click="judgesShown += JUDGES">Show {{ Math.min(JUDGES, view.verification.judges.length - judgesShown) }} more of {{ view.verification.judges.length - judgesShown }} judges</ShowMore>
      <EmptyState v-if="!view.verification.judges.length">No agent has judged this chain yet. A reviewer and a QC judge each change before it is published, and the goal judge checks the result when the Chair asks to finish.</EmptyState>
    </PageSection>
  </div>
</template>

<style scoped>
/* One gate: a card of its own in the section. */
.gate {
  padding: 12px 14px;
  border-radius: 8px;
  background: var(--bg2);
  box-shadow: var(--shadow-card);
}
</style>
