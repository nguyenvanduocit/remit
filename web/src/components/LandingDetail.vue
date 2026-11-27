<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import { bakeFor, checksText, GATE_NAME, PRIORITY_SAYS, queueKey, RETURN_SAYS, REVERT_SAYS, shortSha } from '../landings';
import { releaseName } from '../releases';
import { useStream } from '../streamContext';
import type { Landing, LandingRead, Release } from '../core/generated/types.generated';
import ActorTag from './ActorTag.vue';
import Ago from './Ago.vue';
import Callout from './Callout.vue';
import Icon from './Icon.vue';
import KeyValues from './KeyValues.vue';
import LandingBadge from './LandingBadge.vue';
import Markdown from './Markdown.vue';
import ReleaseBadge from './ReleaseBadge.vue';
import PageSection from './PageSection.vue';
import { Badge } from '@/components/ui/badge';

// One landing of a chain as its record says it (ADR-0114): where it stands, why it went back to its Chair, what took it back or
// is taking it back (who and why), the Chair's description, where it lands and as which version, the gates that approved it, the batch and
// commit it became and the forge's checks on its push, the canary stage it lands as, the release that carried it as that
// release stands now (`release`, the server's). `chain` is the chain's landings, so a
// revert and the landing it takes back link to each other when both are the chain's; the revert on its way is the one the
// server says is taking this one back (`takingBack`). The queue it waits in is a link away.
const props = defineProps<{ landing: Landing; release?: Release; chain: readonly LandingRead[] }>();
const route = useRoute();
const stream = useStream();
const inChain = (id: string) => props.chain.find((l) => l.landing.id === id)?.landing;
const version = computed(() => props.landing.version);
const returned = computed(() => (props.landing.state.kind === 'returned' ? props.landing.state : undefined));
const reverted = computed(() => (props.landing.state.kind === 'reverted' ? props.landing.state : undefined));
const revert = computed(() => (reverted.value ? inChain(reverted.value.revert) : undefined));
const taking = computed(() => props.chain.find((r) => r.takingBack === props.landing.id)?.landing);
const takesBack = computed(() => (props.landing.reverts ? inChain(props.landing.reverts.landing) : undefined));
const linkTo = (l: Landing) => ({ query: { ...route.query, landing: l.id, file: undefined } });
const gates = computed(() => version.value.gates.map((g) => GATE_NAME[g]).join(', '));
const failed = (s?: string) => s === 'failed';
</script>

<template>
  <PageSection :title="version.title" id="landing-record" data-landing-record>
    <template #aside>
      <LandingBadge :landing="landing" />
      <RouterLink v-if="landing.queue" class="inline-flex items-center gap-1 whitespace-nowrap text-blue-text hover:underline" :to="{ name: 'queues', query: { queue: queueKey({ repo: landing.repo, base: landing.queue.base }) } }" data-queue-link>Its queue<Icon name="arrow-right" :size="11" /></RouterLink>
    </template>
    <div class="flex flex-col gap-3.5">
      <Callout v-if="returned" tone="note" icon="arrow-left" data-returned>
        <template #title>Returned to its Chair: {{ RETURN_SAYS[returned.reason] }}</template>
        <p class="m-0 text-[11.5px] text-mut"><Ago :at="landing.updatedAt" /></p>
        <pre v-if="returned.output">{{ returned.output }}</pre>
      </Callout>
      <Callout v-if="reverted" tone="error" icon="repeat" data-reverted>
        <template #title>Taken back: {{ REVERT_SAYS[reverted.cause] }}</template>
        <p class="m-0 flex flex-wrap items-center gap-x-1.5 gap-y-1 text-[12.5px] text-text2">
          <ActorTag :by="reverted.by" :name-of="stream.nameOf" />
          <span v-if="reverted.why" class="min-w-0 [overflow-wrap:anywhere]">{{ reverted.why }}</span>
          <span v-if="landing.revertedAt" class="text-mut">· <Ago :at="landing.revertedAt" /></span>
        </p>
        <p class="m-0 mt-1.5 text-[12.5px] text-text2" data-revert>
          The revert: <RouterLink v-if="revert" class="text-blue-text hover:underline" :to="linkTo(revert)">{{ revert.version.title }}</RouterLink><span v-else class="mono">{{ reverted.revert }}</span>
        </p>
      </Callout>
      <Callout v-else-if="taking?.reverts" tone="note" icon="repeat" data-taking-back>
        <template #title>Being taken back: {{ REVERT_SAYS[taking.reverts.cause] }}</template>
        <p class="m-0 flex flex-wrap items-center gap-x-1.5 gap-y-1 text-[12.5px] text-text2">
          <ActorTag :by="taking.reverts.by" :name-of="stream.nameOf" />
          <span v-if="taking.reverts.why" class="min-w-0 [overflow-wrap:anywhere]">{{ taking.reverts.why }}</span>
        </p>
        <p class="m-0 mt-1.5 flex flex-wrap items-center gap-x-1.5 gap-y-1 text-[12.5px] text-text2" data-revert>
          The revert: <RouterLink class="text-blue-text hover:underline" :to="linkTo(taking)">{{ taking.version.title }}</RouterLink><LandingBadge :landing="taking" />
        </p>
      </Callout>
      <Markdown v-if="version.description" :text="version.description" type="quiet" data-description />
      <KeyValues>
        <template v-if="landing.queue">
          <dt>Lands on</dt>
          <dd>{{ landing.repo }} · <span class="mono">{{ landing.queue.project }} → {{ landing.queue.base }}</span></dd>
        </template>
        <template v-else>
          <dt>Folder</dt>
          <dd><span class="mono">{{ landing.repo }}</span> · worked in place<template v-if="landing.commit">, committed on the branch it is on</template></dd>
        </template>
        <dt>Version</dt>
        <dd>{{ version.number }} · <span class="mono">{{ shortSha(version.from) }}</span><template v-if="version.forked">, forked from <span class="mono">{{ shortSha(version.forked) }}</span></template></dd>
        <template v-if="landing.reverts">
          <dt>Takes back</dt>
          <dd data-takes-back><RouterLink v-if="takesBack" class="text-blue-text hover:underline" :to="linkTo(takesBack)">{{ takesBack.version.title }}</RouterLink><span v-else class="mono">{{ landing.reverts.landing }}</span></dd>
          <dt>Why</dt>
          <dd class="flex flex-wrap items-center gap-x-1.5 gap-y-1">{{ REVERT_SAYS[landing.reverts.cause] }} · <ActorTag :by="landing.reverts.by" :name-of="stream.nameOf" /><span v-if="landing.reverts.why" class="min-w-0 [overflow-wrap:anywhere]">{{ landing.reverts.why }}</span></dd>
        </template>
        <template v-if="landing.priority">
          <dt>Priority</dt>
          <dd>{{ PRIORITY_SAYS[landing.priority] }}<template v-if="landing.priorityWhy"> · {{ landing.priorityWhy }}</template></dd>
        </template>
        <dt>Gates</dt>
        <dd>{{ gates || 'None: a revert puts back a state already judged' }}<template v-if="version.gates.length"> · {{ version.verdicts.length }} approving {{ version.verdicts.length === 1 ? 'verdict' : 'verdicts' }}</template></dd>
        <template v-if="landing.batch">
          <dt>Batch</dt>
          <dd><span class="mono">{{ landing.batch }}</span><template v-if="landing.onto"> on <span class="mono">{{ shortSha(landing.onto) }}</span></template></dd>
        </template>
        <template v-if="landing.commit">
          <dt>Commit</dt>
          <dd class="mono">{{ shortSha(landing.commit) }}</dd>
        </template>
        <template v-if="landing.push">
          <dt>Forge’s checks</dt>
          <dd data-checks>
            <span :class="{ 'text-destructive': failed(landing.checks?.state) }"><Icon v-if="failed(landing.checks?.state)" class="mr-1 align-[-1px]" name="alert" :size="12" />{{ checksText(landing.checks) }}</span> on its push <span class="mono">{{ shortSha(landing.push) }}</span> ·
            {{ checksText(landing.baseChecks) }} on the base before it
          </dd>
        </template>
        <template v-if="landing.readyAt">
          <dt>Ready</dt>
          <dd><Ago :at="landing.readyAt" /></dd>
        </template>
        <template v-if="landing.landedAt">
          <dt>{{ landing.queue ? 'Landed' : landing.commit ? 'Committed' : 'Approved' }}</dt>
          <dd><Ago :at="landing.landedAt" /></dd>
        </template>
        <template v-if="landing.release">
          <dt>Released in</dt>
          <dd class="flex flex-wrap items-center gap-x-1.5 gap-y-1" data-release>
            <RouterLink class="text-blue-text hover:underline" :to="{ name: 'releases', query: { repo: landing.repo, release: landing.release } }">{{ release ? releaseName(release) : landing.release }}</RouterLink>
            <ReleaseBadge v-if="release" :state="release.state" />
          </dd>
        </template>
        <template v-if="landing.reference">
          <dt>Carried over from</dt>
          <dd><a class="text-blue-text hover:underline" :href="landing.reference" target="_blank" rel="noopener">{{ landing.reference }}</a></dd>
        </template>
      </KeyValues>
      <div v-if="version.plan" class="flex flex-col gap-2 border-t border-line pt-3" data-plan>
        <h3 class="flex items-center gap-2 text-[12.5px] font-semibold text-text">A canary stage<Badge v-if="version.plan.undo" variant="warning" size="count">Takes an earlier stage back</Badge></h3>
        <KeyValues>
          <dt>Reaches</dt>
          <dd>{{ version.plan.exposure }}</dd>
          <dt>Guardrail</dt>
          <dd><span class="mono">{{ version.plan.guardrail }}</span> breaches when <span class="mono">{{ version.plan.breach }}</span></dd>
          <dt>Bakes</dt>
          <dd>{{ bakeFor(version.plan.bake) }}</dd>
          <dt>Way back</dt>
          <dd>{{ version.plan.rollback }}</dd>
        </KeyValues>
      </div>
    </div>
  </PageSection>
</template>
