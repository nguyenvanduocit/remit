<script setup lang="ts">
import { computed } from 'vue';
import type { RouteLocationRaw } from 'vue-router';
import Ago from './Ago.vue';
import Fold from './Fold.vue';
import ListState from './ListState.vue';
import PageBody from './PageBody.vue';
import PageSection from './PageSection.vue';
import PageTabs from './PageTabs.vue';
import Tip from './Tip.vue';
import { useApi } from '../core/api';
import type { JudgmentMistake } from '../core/generated/types.generated';
import { useRemote } from '../useRemote';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';

// How Remit's small judgments do (ADR-0118): one section per question its deciders were asked, the questions asked last
// first, each titled by what it is for. It leads with the server's sentence (which model answers it and why, and whether it
// has settled), then the figures it does not repeat (how often it proved right, how often it decides alone, what it
// cost) as the Outcomes tab's figure cards, then its recent mistakes, each said as what happened and linked to the chain it was asked
// in. Its key, threshold and each model's Brier score and AUC stay folded under its details. The server writes all of it
// (judgmentsGet); the page reads it when it opens. The owner steers a question through the Assistant, never here.
const api = useApi();
const report = useRemote(() => ({}), () => api.judgmentsGet());
const questions = computed(() => report.data.value?.questions);
const num = 'text-right tabular-nums';

// Where a mistake opens: the chain it was asked in, at the card it was about. Nothing for one asked in no chain.
const to = (m: JudgmentMistake): RouteLocationRaw | undefined =>
  m.home.startsWith('ch_') ? { name: 'chain', params: { id: m.home, tab: 'live' }, query: { card: m.card || undefined } } : undefined;
// Dollars, to the cent's hundredth: a judgment costs fractions of one.
function usd(n: number): string {
  if (n === 0) return '$0';
  return n < 0.0001 ? '<$0.0001' : `$${n.toFixed(4)}`;
}
const fixed = (n: number | undefined, digits: number) => (n === undefined ? '–' : n.toFixed(digits));
</script>

<template>
  <PageTabs group="results" />
  <PageBody>
    <div class="@container flex flex-col gap-5">
      <ListState what="the judgments" :held="!!questions" :error="report.error.value" :retry="report.retry" :empty="questions?.length === 0">
        <template #empty>No judgment yet: Remit records one each time a point that judges asks its deciders.</template>
        <PageSection v-for="q in questions" :key="q.key" :title="q.title" :id="`q-${q.key}`" :data-question="q.key">
          <p class="m-0 text-[13px] leading-[1.5] text-text"><span data-answered-by>{{ q.answeredBy }}</span> <span data-summary>{{ q.summary }}</span></p>
          <div class="mt-3 grid gap-3 [grid-template-columns:repeat(auto-fit,minmax(170px,1fr))]">
            <article v-for="f in q.figures" :key="f.label" class="kpi" :aria-label="f.label" data-figure>
              <Tip :text="f.hint"><h3 class="cursor-help text-[12.5px] leading-[1.3] font-medium text-text2">{{ f.label }}</h3></Tip>
              <div class="mt-2 flex items-baseline gap-2">
                <span class="text-[26px] leading-none font-semibold tabular-nums" data-value>{{ f.value }}</span>
                <span class="min-w-0 truncate text-[11.5px] text-mut tabular-nums" data-over>{{ f.over }}</span>
              </div>
            </article>
          </div>
          <h3 class="mt-4 mb-1 text-[12px] font-semibold text-text2">Recent mistakes</h3>
          <p v-if="!q.mistakes.length" class="m-0 text-[12.5px] text-mut" data-no-mistakes>None of its answers has proved wrong.</p>
          <Table v-else class="text-[12.5px]" data-mistakes>
            <TableHeader>
              <TableRow><TableHead class="pl-0">What happened</TableHead><TableHead :class="num" class="pr-0">When</TableHead></TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="m in q.mistakes" :key="m.id" data-mistake>
                <TableCell class="pl-0 whitespace-normal [overflow-wrap:anywhere]">
                  <RouterLink v-if="to(m)" class="text-blue-text hover:underline" :to="to(m)!">{{ m.text }}</RouterLink><span v-else>{{ m.text }}</span><span class="text-mut"> · {{ m.sure }}</span>
                </TableCell>
                <TableCell :class="num" class="pr-0 align-top whitespace-nowrap text-mut"><Ago :at="m.at" /></TableCell>
              </TableRow>
            </TableBody>
          </Table>
          <Fold class="mt-3" data-details>
            <template #summary><span class="text-xs text-blue-text">Show the details</span></template>
            <dl class="mt-2 grid grid-cols-[auto_1fr] items-center gap-x-4 gap-y-1.5 text-[12.5px]">
              <template v-for="f in q.details.facts" :key="f.label">
                <dt class="text-mut">{{ f.label }}</dt>
                <dd class="m-0 text-text2 [overflow-wrap:anywhere]">{{ f.value }}</dd>
              </template>
            </dl>
            <Table class="mt-2 text-[12.5px]" data-deciders>
              <TableHeader>
                <TableRow><TableHead class="pl-0">Model</TableHead><TableHead>Role</TableHead><TableHead :class="num">Answers</TableHead><TableHead :class="num">Results known</TableHead><TableHead :class="num">Brier score</TableHead><TableHead :class="num">AUC</TableHead><TableHead :class="num" class="pr-0">Cost</TableHead></TableRow>
              </TableHeader>
              <TableBody>
                <TableRow v-for="d in q.details.deciders" :key="d.decider" :data-decider="d.decider">
                  <TableCell class="pl-0 font-medium">{{ d.name }} <span class="font-mono text-[11.5px] font-normal text-mut">{{ d.decider }}</span></TableCell>
                  <TableCell>{{ d.role }}</TableCell>
                  <TableCell :class="num">{{ d.judgments }}</TableCell>
                  <TableCell :class="num">{{ d.outcomes }}<span v-if="d.unknown" class="text-mut"> · {{ d.unknown }} unknown</span></TableCell>
                  <TableCell :class="num">{{ fixed(d.brier, 3) }}</TableCell>
                  <TableCell :class="num">{{ fixed(d.auc, 2) }}</TableCell>
                  <TableCell :class="num" class="pr-0">{{ usd(d.costUsd) }}</TableCell>
                </TableRow>
              </TableBody>
            </Table>
          </Fold>
        </PageSection>
      </ListState>
    </div>
  </PageBody>
</template>

<style scoped>
/* A figure: the Outcomes tab's figure card (OutcomeKpi), its name over its value and what the value is over. */
.kpi {
  display: flex;
  flex-direction: column;
  min-width: 0;
  padding: 12px 14px;
  border-radius: 8px;
  background: var(--panel);
  box-shadow: var(--shadow-card);
}
</style>
