<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import { catalog, chainRecords, projectOf, standingOf } from '../catalog';
import { useClock } from '../clock';
import { chainsByKeyResult, peopleOf, personOf, stateOf } from '../objectiveTree';
import { checkFacts, checkText, horizonText, LOOK, personText, RETIRED } from '../objectiveLook';
import type { Accountable, KeyResult } from '../core/generated/types.generated';
import { fullTime, plural, timed } from '../ui';
import Ago from './Ago.vue';
import ChainStandingBadge from './ChainStandingBadge.vue';
import ChangeHistory from './ChangeHistory.vue';
import EmptyState from './EmptyState.vue';
import Icon from './Icon.vue';
import KeyValues from './KeyValues.vue';
import PageBody from './PageBody.vue';
import PageHead from './PageHead.vue';
import PageSection from './PageSection.vue';
import StandingBadge from './StandingBadge.vue';
import Tip from './Tip.vue';
import { Button } from '@/components/ui/button';

// The objective or key result the address names (ObjectivesView), read-only, beside the tree. An objective: who it is for, what it
// aims at, the project it belongs to, how far ahead it looks, its key results with where each stands, what it must not push past, what stands in for what
// cannot be read yet, what it does not know, and its history. A key result: who answers for it and who judges it, where it
// stands, how it is checked and the chains that work toward it. Nothing here changes either: the owner tells the Assistant.
const props = defineProps<{ objective: string; keyResult: string }>();
defineEmits<{ close: [] }>();
const route = useRoute();

const view = computed(() => catalog.objectives.find((v) => v.objective.id === props.objective && !v.objective.task));
const o = computed(() => view.value?.objective);
// The view carries the whole tree of key results: the owner's (no parent) and those that refine them.
const tree = computed(() => view.value?.keyResults ?? []);
const committed = computed(() => tree.value.filter((k) => !k.parent));
const kr = computed(() => (props.keyResult ? tree.value.find((k) => k.id === props.keyResult) : undefined));
const refines = computed(() => (kr.value?.parent ? tree.value.find((k) => k.id === kr.value?.parent) : undefined));
const refinedBy = (id: string) => tree.value.filter((k) => k.parent === id);
const stalledWhy = computed(() => (kr.value ? (view.value?.stalled[kr.value.id] ?? '') : ''));
// What is shown: nothing named yet, the named objective or key result, or a name the replica does not list.
const shown = computed(() => (!props.objective ? 'none' : !o.value || (props.keyResult && !kr.value) ? 'gone' : kr.value ? 'keyResult' : 'objective'));

const people = computed(() => peopleOf(catalog.employees));
const who = (a: Accountable) => personText(personOf(people.value, a));
const served = computed(() => chainsByKeyResult(chainRecords.value));
const chainsOf = (id: string) => served.value.get(`${props.objective}/${id}`) ?? [];

// A horizon passes by the minute, not by the second the clock ticks in.
const clock = useClock();
const minute = computed(() => Math.floor(clock.value.getTime() / 60_000) * 60_000);
const objectiveLook = computed(() => LOOK[view.value ? stateOf(view.value, minute.value) : 'active']);
const keyResultLook = (k: KeyResult) => (k.retired ? RETIRED : LOOK[view.value?.status[k.id] ?? 'unserved']);

// A link keeps what refines the screen (the search, the filters, what is unfolded) and names the key result, or none.
const objectiveLink = (keyResult?: string) => ({ name: 'objectives', params: { id: props.objective }, query: { ...route.query, kr: keyResult } });
</script>

<template>
  <div class="flex min-w-0 flex-1 flex-col" :data-detail="shown">
    <template v-if="o && (shown === 'objective' || (shown === 'keyResult' && kr))">
      <PageHead :title="kr ? kr.text : o.outcome || o.id">
        <template #standing>
          <StandingBadge :variant="(kr ? keyResultLook(kr) : objectiveLook).tone" :icon="(kr ? keyResultLook(kr) : objectiveLook).icon" class="flex-none" data-status>{{ (kr ? keyResultLook(kr) : objectiveLook).label }}</StandingBadge>
        </template>
        <template #end><Button variant="ghost" size="icon-sm" aria-label="Close the details" @click="$emit('close')"><Icon name="x" :size="14" /></Button></template>
      </PageHead>
      <PageBody>
        <div v-if="kr" class="flex flex-col gap-3.5">
          <PageSection title="Who answers for it" id="kr-who" data-who>
            <KeyValues>
              <dt>Answers for it</dt>
              <dd :class="{ 'text-mut': personOf(people, kr.owner).retired }">{{ who(kr.owner) }}</dd>
              <dt>Judges it</dt>
              <dd :class="{ 'text-mut': personOf(people, kr.judge).retired }">{{ who(kr.judge) }}</dd>
            </KeyValues>
            <p v-if="kr.retired" class="mt-3 text-xs text-mut" data-retired-note>Retired: an amendment dropped it, or what it refines is retired or met. Its chains finish and no new one opens; it counts for nothing.</p>
          </PageSection>
          <PageSection v-if="stalledWhy" title="Stalled" id="kr-stalled" data-stalled>
            <p class="m-0 text-text2 [overflow-wrap:anywhere]">{{ stalledWhy }}</p>
            <p class="mt-2 mb-0 text-xs text-mut">Tell the Assistant to lift it once something is different, or to refine it into parts that can progress.</p>
          </PageSection>
          <PageSection v-if="refines || refinedBy(kr.id).length" title="Refinement" :count="refinedBy(kr.id).length" id="kr-refinement" data-refinement>
            <template v-if="!kr.check" #aside>met when every key result refining it is</template>
            <p v-if="refines" class="mt-0 mb-2 text-xs text-mut" data-refines>refines <RouterLink class="text-blue-text hover:underline" :to="objectiveLink(refines.id)">{{ refines.text }}</RouterLink></p>
            <ul v-if="refinedBy(kr.id).length" class="m-0 flex list-none flex-col gap-2 p-0">
              <li v-for="k in refinedBy(kr.id)" :key="k.id" class="flex min-w-0 items-center gap-2" :data-child="k.id">
                <RouterLink class="min-h-6 min-w-0 flex-1 truncate leading-6 text-blue-text hover:underline" :to="objectiveLink(k.id)">{{ k.text }}</RouterLink>
                <StandingBadge :variant="keyResultLook(k).tone" :icon="keyResultLook(k).icon" class="flex-none">{{ keyResultLook(k).label }}</StandingBadge>
              </li>
            </ul>
          </PageSection>
          <PageSection title="How it is checked" id="kr-check" data-check>
            <KeyValues>
              <template v-for="[label, value] in checkFacts(kr.check)" :key="label"><dt>{{ label }}</dt><dd>{{ value }}</dd></template>
            </KeyValues>
          </PageSection>
          <PageSection title="Chains" :count="chainsOf(kr.id).length" id="kr-chains" data-chains>
            <ul v-if="chainsOf(kr.id).length" class="m-0 flex list-none flex-col gap-2 p-0">
              <li v-for="c in chainsOf(kr.id)" :key="c.id" class="flex min-w-0 items-center gap-2">
                <RouterLink class="min-h-6 min-w-0 flex-1 truncate leading-6 text-blue-text hover:underline" :to="{ name: 'chain', params: { id: c.id } }" :data-chain="c.id">{{ c.title }}</RouterLink>
                <ChainStandingBadge :standing="standingOf(c.id)" />
              </li>
            </ul>
            <EmptyState v-else>No chain works toward it yet.</EmptyState>
          </PageSection>
        </div>
        <div v-else class="flex flex-col gap-3.5">
          <PageSection title="About" id="objective-about" data-about>
            <KeyValues>
              <dt>Project</dt>
              <dd v-if="o.project" data-project><RouterLink class="text-blue-text hover:underline" :to="{ name: 'projects', params: { id: o.project } }">{{ projectOf(o.project)?.name ?? o.project }}</RouterLink></dd>
              <dd v-else class="text-mut" data-project>none</dd>
              <dt>Customer</dt><dd>{{ o.customer || 'not named' }}</dd>
              <dt>North star</dt><dd>{{ o.northStar || 'none' }}</dd>
              <dt>Scope</dt><dd>{{ o.scope || 'none' }}</dd>
              <dt>Owner</dt><dd :class="{ 'text-mut': personOf(people, o.owner).retired }">{{ who(o.owner) }}</dd>
              <dt>Horizon</dt><dd>{{ horizonText(o.horizon) || 'none' }}</dd>
              <dt>Made</dt><dd><Tip :text="fullTime(o.createdAt)"><time :datetime="o.createdAt"><Ago :at="o.createdAt" /></time></Tip></dd>
              <dt>Id</dt><dd class="mono">{{ o.id }}</dd>
            </KeyValues>
          </PageSection>
          <PageSection title="Key results" :count="committed.filter((k) => !k.retired).length" id="objective-keyresults" data-keyresults>
            <ul v-if="committed.length" class="m-0 flex list-none flex-col gap-2.5 p-0">
              <li v-for="k in committed" :key="k.id" class="flex flex-col gap-1 rounded-lg bg-bg2 px-3 py-2.5" :data-kr="k.id">
                <div class="flex min-w-0 items-start gap-2">
                  <RouterLink class="min-w-0 flex-1 font-medium text-text [overflow-wrap:anywhere] hover:underline" :to="objectiveLink(k.id)">{{ k.text }}</RouterLink>
                  <StandingBadge :variant="keyResultLook(k).tone" :icon="keyResultLook(k).icon" class="flex-none">{{ keyResultLook(k).label }}</StandingBadge>
                </div>
                <p class="m-0 text-xs text-mut">answers for {{ who(k.owner) }} · judged by {{ who(k.judge) }} · {{ checkText(k.check?.kind) }} · {{ plural(chainsOf(k.id).length, 'chain') }}<template v-if="refinedBy(k.id).length"> · {{ refinedBy(k.id).length }} refining it</template><template v-if="view?.stalled[k.id]"> · stalled</template></p>
              </li>
            </ul>
            <EmptyState v-else>No key result yet: the Assistant adds one when you tell it how this objective is measured.</EmptyState>
          </PageSection>
          <PageSection v-if="o.guardrails?.length" title="Guardrails" :count="o.guardrails.length" id="objective-guardrails" data-guardrails>
            <template #aside>measures the work must not push past</template>
            <ul class="m-0 flex list-none flex-col gap-2.5 p-0">
              <li v-for="g in o.guardrails" :key="g.kpi" class="flex flex-col gap-0.5" data-guardrail>
                <span class="text-text">{{ g.kpi }}</span>
                <span class="text-xs text-mut">bound: <span class="text-text2">{{ g.condition }}</span></span>
                <span class="text-xs text-mut"><template v-if="g.signal">read by <span class="mono text-text2">{{ g.signal }}</span></template><template v-else>no signal reads it yet, so nothing checks it</template></span>
              </li>
            </ul>
          </PageSection>
          <PageSection v-if="o.surrogates?.length" title="Surrogates" :count="o.surrogates.length" id="objective-surrogates" data-surrogates>
            <template #aside>stand-ins for what cannot be read yet</template>
            <ul class="m-0 flex list-none flex-col gap-2 p-0">
              <li v-for="s in o.surrogates" :key="s.kpi" class="flex flex-col gap-0.5">
                <b>{{ s.kpi }}</b>
                <span class="text-xs text-mut">
                  <template v-if="s.owner">owned by {{ who(s.owner) }}</template><template v-if="s.reads"> · reads {{ s.reads }}</template><template v-if="timed(s.expires)"> · holds until {{ s.expires }}</template>
                </span>
              </li>
            </ul>
          </PageSection>
          <PageSection v-if="o.unknown?.length" title="Unknowns" :count="o.unknown.length" id="objective-unknowns" data-unknowns>
            <ul class="m-0 flex list-none flex-col gap-2 p-0">
              <li v-for="u in o.unknown" :key="u.text" class="flex flex-col gap-0.5">
                <span>{{ u.text }}</span>
                <span v-if="u.question" class="text-xs text-mut">to settle it: {{ u.question }}</span>
              </li>
            </ul>
          </PageSection>
          <ChangeHistory kind="objective" :name="o.id" :rev="o.rev" />
        </div>
      </PageBody>
    </template>
    <PageBody v-else><EmptyState>{{ shown === 'none' ? 'Open an objective or a key result to read its details.' : 'This objective or key result is not listed.' }}</EmptyState></PageBody>
  </div>
</template>
