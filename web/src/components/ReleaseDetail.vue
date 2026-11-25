<script setup lang="ts">
import { computed } from 'vue';
import { bakeFor, shortSha } from '../landings';
import { releaseName } from '../releases';
import { useStream } from '../streamContext';
import type { Landing, Release } from '../core/generated/types.generated';
import ActorTag from './ActorTag.vue';
import Ago from './Ago.vue';
import Callout from './Callout.vue';
import KeyValues from './KeyValues.vue';
import PageSection from './PageSection.vue';
import ReleaseBadge from './ReleaseBadge.vue';
import ReleaseSteps from './ReleaseSteps.vue';
import Until from './Until.vue';

// One release as its record says it (ADR-0116): where it stands, what holds it, who took it back and why with what its way
// back ran, the version and links its procedure reported, the commits it released and the landings it carried (`carried`,
// oldest first, each a link to its chain's record of it), what it bakes under and until when, and each step its procedure ran
// with what it said.
const props = defineProps<{ release: Release; carried: readonly Landing[] }>();
const stream = useStream();
const takenBack = computed(() => (props.release.state.kind === 'taken_back' ? props.release.state : undefined));
const toLanding = (l: Landing) => ({ name: 'chain', params: { id: l.chainId, tab: 'landings' }, query: { landing: l.id } });
</script>

<template>
  <PageSection :title="`Release ${releaseName(release)}`" id="release-record" data-release-record>
    <template #aside><ReleaseBadge :state="release.state" /></template>
    <div class="flex flex-col gap-3.5">
      <Callout v-if="takenBack" tone="error" icon="repeat" data-taken-back>
        <template #title>Taken back</template>
        <p class="m-0 flex flex-wrap items-center gap-x-1.5 gap-y-1 text-[12.5px] text-text2">
          <ActorTag :by="takenBack.by" :name-of="stream.nameOf" />
          <span v-if="takenBack.why" class="min-w-0 [overflow-wrap:anywhere]">{{ takenBack.why }}</span>
          <span v-if="release.endedAt" class="text-mut">· <Ago :at="release.endedAt" /></span>
        </p>
      </Callout>
      <Callout v-if="release.waits" tone="note" icon="clock" data-waits>
        <template #title>Waiting to run again</template>
        <p class="m-0 text-[12.5px] text-text2 [overflow-wrap:anywhere]">{{ release.waits }}</p>
      </Callout>
      <KeyValues>
        <dt>Releases</dt>
        <dd>{{ release.repo }} · <span class="mono">{{ release.project }} → {{ release.base }}</span></dd>
        <dt>Commit</dt>
        <dd><span class="mono">{{ shortSha(release.commit) }}</span><template v-if="release.previous">, after <span class="mono">{{ shortSha(release.previous) }}</span></template><template v-else>, the repository’s first release</template></dd>
        <template v-if="carried.length">
          <dt>Carries</dt>
          <dd class="flex flex-col gap-0.5" data-carried>
            <RouterLink v-for="l in carried" :key="l.id" class="text-blue-text hover:underline [overflow-wrap:anywhere]" :to="toLanding(l)">{{ l.version.title }}</RouterLink>
          </dd>
        </template>
        <template v-if="release.links.length">
          <dt>Reached at</dt>
          <dd class="flex flex-col gap-0.5" data-links><a v-for="l in release.links" :key="l" class="text-blue-text hover:underline [overflow-wrap:anywhere]" :href="l" target="_blank" rel="noopener">{{ l }}</a></dd>
        </template>
        <template v-if="release.tries > 1">
          <dt>Ran</dt>
          <dd data-tries>{{ release.tries }} times</dd>
        </template>
        <dt>Started</dt>
        <dd><Ago :at="release.createdAt" /></dd>
        <template v-if="release.state.kind === 'baking' && release.bakeUntil">
          <dt>Bakes</dt>
          <dd data-bakes><Until :at="release.bakeUntil" /></dd>
        </template>
        <template v-if="release.endedAt">
          <dt>Ended</dt>
          <dd><Ago :at="release.endedAt" /></dd>
        </template>
      </KeyValues>
      <div v-if="release.guards.length" class="flex flex-col gap-2 border-t border-line pt-3" data-guards>
        <h3 class="text-[12.5px] font-semibold text-text">What it bakes under</h3>
        <ul class="m-0 flex list-none flex-col gap-1 p-0 text-[12.5px] text-text2">
          <li v-for="g in release.guards" :key="g.signal + g.breach"><span class="mono">{{ g.signal }}</span> breaches when <span class="mono">{{ g.breach }}</span> · watched {{ bakeFor(g.bake) }}</li>
        </ul>
      </div>
      <ReleaseSteps v-if="release.steps.length" title="Its procedure" :steps="release.steps" data-steps />
      <ReleaseSteps v-if="release.back.length" title="Its way back" :steps="release.back" data-back />
    </div>
  </PageSection>
</template>
