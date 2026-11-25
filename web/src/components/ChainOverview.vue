<script setup lang="ts">
import { computed } from 'vue';
import { capabilityLabel, catalog, projectOf, standingOf } from '../catalog';
import { decisionRows } from '../decisionRows';
import { chairOf } from '../openChain';
import type { ChainView } from '../core/generated/types.generated';
import { clockAt, fullTime } from '../ui';
import Ago from './Ago.vue';
import Elapsed from './Elapsed.vue';
import AgentPlan from './AgentPlan.vue';
import CardDecisions from './CardDecisions.vue';
import ChainStandingBadge from './ChainStandingBadge.vue';
import ChainWatches from './ChainWatches.vue';
import Icon from './Icon.vue';
import KeyValues from './KeyValues.vue';
import Markdown from './Markdown.vue';
import PageBody from './PageBody.vue';
import PageSection from './PageSection.vue';
import PageGrid from './PageGrid.vue';

// The chain at a glance: where it stands, as the server says it, in a band over everything (its badge, why it waits and since
// when, when its Chair checks back, its place in the queue), then its cards: the goal, the facts of how it runs (its project, kind,
// directory, who started it, its agents, and where Datadog shows
// what it did when the server sends there), the Chair's plan for it, the Chair's summary of the result, what it watches, and
// what was decided in it and why. Wide enough, the cards stand two abreast.
const props = defineProps<{ view: ChainView }>();
const chain = computed(() => props.view.chain.chain);
const decisions = computed(() => props.view.decisions.map((d) => d.decision));
const plan = computed(() => chairOf(props.view)?.plan ?? []);
const agents = computed(() => props.view.cards.filter((c) => c.id !== chain.value.chairId).length);
// Where it stands in the queue, and what holds it there (held for the owner's review), as the server says.
const standing = computed(() => standingOf(chain.value.id));
const queued = computed(() => chain.value.status === 'queued');
// The server's words: the state first, then what it says beside it (Not done · not proven: the live effect). The badge
// wears the state, in one word never cut, the rest follows it on the status line; why it stands there reads whole beneath.
const words = computed(() => standing.value.text.split(' · '));
const state = computed(() => words.value[0]);
const rest = computed(() => words.value.slice(1).join(' · '));
// Why it stands there, unless the status line already says it (Not done · not proven: <why>).
const why = computed(() => (standing.value.why && !rest.value.includes(standing.value.why) ? standing.value.why : ''));
</script>

<template>
  <PageBody>
    <div class="@container flex flex-col gap-5">
      <!-- Where the chain stands, in the server's words: the one thing to read first. -->
      <div class="band" :class="{ needs: !!standing.wait }" role="region" aria-label="Where the chain stands" data-standing>
        <div class="flex flex-wrap items-center gap-x-2.5 gap-y-1">
          <ChainStandingBadge :standing="standing" class="flex-none">{{ state }}</ChainStandingBadge>
          <span v-if="rest" class="min-w-0 text-[12.5px] font-medium [overflow-wrap:anywhere]" data-rest>{{ rest }}</span>
          <span v-if="standing.wait" class="ml-auto flex flex-none items-center gap-1 text-[11.5px] text-mut" :title="fullTime(standing.wait.since)" data-since><Icon name="clock" :size="11" />waiting <Elapsed :from="standing.wait.since" /></span>
          <span v-else-if="standing.checkBackAt" class="ml-auto flex flex-none items-center gap-1 text-[11.5px] text-mut" :title="fullTime(standing.checkBackAt)" data-check-back><Icon name="clock" :size="11" />checks back {{ clockAt(standing.checkBackAt) }}</span>
        </div>
        <div v-if="queued || standing.wait || why || standing.working || !rest" class="text-[13px] leading-[1.5] text-text2">
          <template v-if="queued"><b>Position #{{ standing.place }}</b> in the queue. Remit starts it automatically when there is room ({{ catalog.workload?.running }}/{{ catalog.workload?.capacity }} running) and a provider that answers.<template v-if="standing.why"> <b>Why it waits:</b><Markdown class="mt-1" :text="standing.why" chain-tab="" /></template></template>
          <template v-else-if="standing.wait"><b>{{ standing.wait.label }}:</b> {{ standing.wait.ask }}</template>
          <Markdown v-else-if="why" :text="why" chain-tab="" />
          <template v-else-if="standing.working">The agents are at work.</template>
          <template v-else>Nothing waits on you here.</template>
        </div>
      </div>
      <PageGrid>
        <PageSection title="Goal" id="goal" data-goal>
          <Markdown type="quiet" :text="chain.goal" />
        </PageSection>
        <PageSection title="How it runs" id="facts" data-facts>
          <KeyValues>
            <template v-if="standing.project">
              <dt>Project</dt><dd data-project><RouterLink class="text-blue-text hover:underline" :to="{ name: 'projects', params: { id: standing.project } }">{{ projectOf(standing.project)?.name ?? standing.project }}</RouterLink></dd>
            </template>
            <dt>Kind</dt><dd data-kind>{{ chain.kind }}</dd>
            <template v-if="chain.capabilities?.length"><dt>Capabilities</dt><dd>{{ chain.capabilities.map(capabilityLabel).join(' · ') }}</dd></template>
            <dt>Started by</dt><dd data-started-by>{{ standing.from }}</dd>
            <dt>Started</dt><dd :title="fullTime(chain.createdAt)"><Ago :at="chain.createdAt" /></dd>
            <dt>Directory</dt><dd class="mono">{{ chain.workdir }}</dd>
            <dt>Agents</dt><dd>{{ agents }}</dd>
            <template v-if="view.datadog.length">
              <dt>Datadog</dt><dd data-datadog>See this chain's <template v-for="(l, i) in view.datadog" :key="l.label"><template v-if="i"> · </template><a class="text-blue-text hover:underline" :href="l.url" target="_blank" rel="noopener" :aria-label="`This chain's ${l.label} in Datadog (opens in a new tab)`">{{ l.label }}</a></template></dd>
            </template>
          </KeyValues>
        </PageSection>
        <PageSection v-if="chain.summary" title="Result, as the Chair summarised it" id="result" class="@[1100px]:col-span-2" data-result>
          <Markdown type="quiet" :text="chain.summary" />
        </PageSection>
        <PageSection v-if="plan.length" title="The Chair's plan" :count="plan.length" id="plan" data-plan>
          <template #aside>{{ plan.filter((p) => p.status === 'completed').length }}/{{ plan.length }} done</template>
          <AgentPlan :plan="plan" bare />
        </PageSection>
        <ChainWatches :chain-id="chain.id" />
        <PageSection title="Decisions" :count="decisionRows(decisions).rows.length" id="decisions" class="@[1100px]:col-span-2" data-decisions>
          <template #aside>Newest first</template>
          <CardDecisions :decisions="decisions" />
        </PageSection>
      </PageGrid>
    </div>
  </PageBody>
</template>

<style scoped>
/* The band: a status line (the badge, the rest of the server's words, the clock), then why, the width of the band; on a
   surface, and one that waits on the owner sits on his orange. */
.band {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 14px;
  border-radius: 8px;
  background: var(--panel2);
}
.band.needs {
  background: var(--orange-bg);
}
</style>
