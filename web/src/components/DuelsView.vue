<script setup lang="ts">
import { computed } from 'vue';
import { useApi } from '../core/api';
import { duelMoves } from '../duels';
import type { WorkDuels } from '../core/generated/types.generated';
import { useSayToAssistant } from '../sayToAssistant';
import { plural } from '../ui';
import { useRemote } from '../useRemote';
import AgentBadge from './AgentBadge.vue';
import Callout from './Callout.vue';
import EmptyPage from './EmptyPage.vue';
import EmptyState from './EmptyState.vue';
import ListState from './ListState.vue';
import Icon from './Icon.vue';
import ItemCard from './ItemCard.vue';
import PageTabs from './PageTabs.vue';
import Tip from './Tip.vue';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

// How lineages (the agents employees are hired from) compete, as the server lays it out (duelStandings): one column per kind of
// work holding each lineage's rating, the highest first, with the count of duels it rests on; over the columns, a note saying for
// each kind of work whether duels there go on or are paused, why, and how often the owner left the judge's picks standing there. Read when the page opens and
// again as picks and overturns are pushed.
// Nothing is set here: the share of picks duels need is the coach's, so a paused kind of work offers the words to ask for it in the
// Assistant's box.
const api = useApi();
const sayToAssistant = useSayToAssistant();
// Read when the page opens, and again whenever a judge's pick or an overturn is pushed (duels.ts).
const standings = useRemote(
  () => ({ moves: duelMoves() }),
  () => api.duelStandings(),
);
// Each kind of work with its lineages' duel ratings, in the order the server stands them.
const works = computed(() => (standings.data.value ?? []).map((w) => ({ ...w, ratings: w.standings.map((l) => l.duel) })));
const agreement = (w: WorkDuels) =>
  w.judged ? `The judge agrees with you on ${w.judged - w.overturned} of its last ${plural(w.judged, 'pick')} here (${Math.round((100 * (w.judged - w.overturned)) / w.judged)}%).` : 'No pick judged here yet.';
const askToResume = (w: WorkDuels) => sayToAssistant(`Duels of ${w.work} are paused. Should the coach lower the share of the judge's picks I must leave standing there?`);
</script>

<template>
  <section class="flex min-h-0 flex-1 flex-col overflow-hidden" aria-label="Duels">
    <PageTabs group="crew" />
    <div v-if="!standings.data.value || standings.error.value" class="px-4 pt-3">
      <ListState what="where duels stand" :held="!!standings.data.value" :error="standings.error.value" :retry="standings.retry" />
    </div>
    <template v-if="standings.data.value">
    <div v-if="!works.length" class="flex min-h-0 min-w-0 flex-1 flex-col">
      <EmptyPage title="No duel yet" icon="scale" say="Have the Chairs contest their workers' cards with workers hired from another agent">
        <template #what>A duel puts two employees hired from different agents on the same brief. A judge picks the better report, and you may overturn its pick. Each kind of work rates its lineages by the duels they fought there.</template>
        <template #how>
          <li>A Chair contests a worker's card: a kept worker of another agent works the same brief on a card beside it.</li>
          <li>Once both have reported, a judge picks one; the pick shows among the chain's decisions, where you may overturn it.</li>
          <li>Here you read each lineage's rating with its count of duels, and how often you left the judge's picks standing.</li>
        </template>
      </EmptyPage>
    </div>
    <div v-else class="flex min-h-0 min-w-0 flex-1 flex-col">
      <!-- What the board does not say in its columns: whether duels go on in each kind of work and how often the owner left the judge's picks standing there, and for a paused one why and the words to ask the coach in. -->
      <Callout tone="note" class="mx-4 mt-2.5" data-standing>
        <template #title>Where the judge's picks stand</template>
        <p class="m-0 text-xs text-text2" data-legend>A lineage's rating is read with the duels it rests on: 1500 is even, and over few duels a rating is rough.</p>
        <ul class="m-0 mt-2 flex list-none flex-col gap-2 p-0">
          <li v-for="w in works" :key="w.work" class="flex flex-col items-start gap-1 text-xs text-text2" :data-work-state="w.work" :data-paused="w.paused ? '' : undefined">
            <span class="flex flex-wrap items-center gap-x-2 gap-y-1">
              <span class="caps text-text">{{ w.work }}</span>
              <Badge v-if="w.paused" variant="warning" data-duel-state><Icon name="pause" :size="11" />Paused</Badge>
              <Badge v-else variant="success" data-duel-state><Icon name="play" :size="11" />Duels go on</Badge>
              <span data-agreement>{{ agreement(w) }}</span>
            </span>
            <template v-if="w.paused">
              <span class="[overflow-wrap:anywhere]" data-paused-why>{{ w.paused }}</span>
              <Button variant="outline" size="xs" data-ask-resume @click="askToResume(w)">Ask the Assistant</Button>
            </template>
          </li>
        </ul>
      </Callout>
      <div class="grid min-h-0 flex-1 auto-cols-[minmax(280px,360px)] grid-flow-col gap-2.5 overflow-x-auto px-4 pt-2.5 pb-3.5">
        <section v-for="w in works" :key="w.work" class="flex min-h-0 flex-col rounded-lg bg-bg2 p-2" :aria-label="`Duels in ${w.work}`" :data-work="w.work">
          <h2 class="caps flex items-center gap-[7px] px-1.5 pt-1 pb-2.5">
            <Icon class="text-blue-mark" name="scale" :size="12" />
            <Tip :text="w.work" when-clipped><span class="min-w-0 truncate">{{ w.work }}</span></Tip>
            <Badge variant="quiet" size="count" class="ml-auto tracking-normal" :aria-label="plural(w.ratings.length, 'lineage')" data-count>{{ w.ratings.length }}</Badge>
          </h2>
          <ol v-if="w.ratings.length" class="m-0 flex min-h-0 list-none flex-col gap-2 overflow-auto p-0" :aria-label="`Lineages in ${w.work}, the highest rated first`">
            <li v-for="r in w.ratings" :key="r.template" :data-lineage="r.template">
              <ItemCard :to="{ name: 'agents', params: { agentId: r.template } }" :on="false" :title="r.template">
                <template #chip><AgentBadge :agent="r.template" :size="16" /></template>
                <template #badge><Badge variant="outline" class="font-mono" data-rating>rated {{ r.rating }}</Badge></template>
                <template #meta><span data-duels>over {{ plural(r.duels, 'duel') }} · won {{ r.won }}</span></template>
              </ItemCard>
            </li>
          </ol>
          <EmptyState v-else class="py-4 text-center text-xs">No duel judged here yet: a judge rates a lineage once a duel in this work is decided.</EmptyState>
        </section>
      </div>
    </div>
    </template>
  </section>
</template>
