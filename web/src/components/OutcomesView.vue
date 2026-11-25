<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import Icon from './Icon.vue';
import OutcomeInsights from './OutcomeInsights.vue';
import OutcomeKpi from './OutcomeKpi.vue';
import OutcomeSpanPicker from './OutcomeSpanPicker.vue';
import OutcomeTables from './OutcomeTables.vue';
import PageBody from './PageBody.vue';
import PageSection from './PageSection.vue';
import PageTabs from './PageTabs.vue';
import { useApi } from '../core/api';
import { catalog } from '../catalog';
import { useQueryRef } from '../nav';
import type { OutcomeGroupKey, OutcomeRange, OutcomeWindow } from '../core/generated/types.generated';
import { useRemote } from '../useRemote';
import ListState from './ListState.vue';

// The overall quality of the agents, across every chain: the analyst's insights and their evidence first when available, then the KPIs, a section per group with its cards in one row of equal widths, what the
// analyst made of them, and the counts by role, model and kind of chain. The server computes all of it (outcomesGet), so this
// page and the analyst read the same figures. It is read when the page opens, again when the analyst says something new
// or its chain moves, and again when the owner picks another time range or window (kept in the address: ?last=, ?window=).
const api = useApi();
const analystId = ref('');
const analystMoved = computed(() => {
  const c = catalog.chains.find((x) => x.chain.id === analystId.value);
  return c ? `${c.chain.status} ${c.checkBack?.at ?? ''}` : '';
});
const last = useQueryRef<OutcomeRange>('last', '90d');
const win = useQueryRef<OutcomeWindow>('window', '7d');
const report = useRemote(
  () => ({ insights: catalog.insights.length, analyst: analystMoved.value, last: last.value, window: win.value }),
  (a) => api.outcomesGet(a.last, a.window),
);
watch(() => report.data.value?.analyst?.chainId, (id) => (analystId.value = id ?? ''));
const outcomes = computed(() => report.data.value);
// What each group is about, beside its name, and its icon.
const GROUP: Record<OutcomeGroupKey, { icon: string; about: string }> = {
  headline: { icon: 'scale', about: 'the landings taken back for each confirmed result, read with how many results were confirmed' },
  quality: { icon: 'check', about: 'what the agents deliver, as their judges and the owner rule on it' },
  landing: { icon: 'merge', about: 'how changes reach their base: the wait in the queue, what it sends back, how many each push carries' },
  reliability: { icon: 'alert', about: 'what broke down before it was judged' },
};
</script>

<template>
  <PageTabs group="results">
    <template v-if="outcomes" #end><OutcomeSpanPicker v-model:last="last" v-model:window="win" :span="outcomes.report.span" /></template>
  </PageTabs>
  <PageBody>
    <div class="@container flex flex-col gap-5">
      <ListState what="the outcomes" :held="!!outcomes" :error="report.error.value" :retry="report.retry">
        <template v-if="outcomes">
          <OutcomeInsights v-if="catalog.insights.length" :analyst="outcomes.analyst" />
          <PageSection v-for="g in outcomes.report.groups" :key="g.key" :title="g.label" :id="`group-${g.key}`" :data-group="g.key">
            <template #aside><Icon class="text-mut" :name="GROUP[g.key]?.icon ?? 'dot'" :size="12" aria-hidden="true" /><span class="min-w-0 truncate">{{ GROUP[g.key]?.about }}</span></template>
            <div class="grid gap-3 [grid-template-columns:repeat(auto-fit,minmax(170px,1fr))]">
              <OutcomeKpi v-for="k in g.kpis" :key="k.key" :kpi="k" :span="outcomes.report.span" />
            </div>
          </PageSection>
          <OutcomeInsights v-if="!catalog.insights.length" :analyst="outcomes.analyst" />
          <OutcomeTables :report="outcomes.report" />
        </template>
      </ListState>
    </div>
  </PageBody>
</template>
