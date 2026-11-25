<script setup lang="ts">
import { computed, ref, watchEffect } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { standingOf } from '../catalog';
import { defaultTab } from '../nav';
import { actorName } from '../stream';
import { useStream } from '../streamContext';
import { clockAt } from '../ui';
import type { ChainView } from '../core/generated/types.generated';
import AgentTree from './AgentTree.vue';
import ChainActions from './ChainActions.vue';
import CheckBackChip from './CheckBackChip.vue';
import ChainLandings from './ChainLandings.vue';
import ChainOverview from './ChainOverview.vue';
import ChainStandingBadge from './ChainStandingBadge.vue';
import ChainVerification from './ChainVerification.vue';
import LiveView from './LiveView.vue';
import PageBody from './PageBody.vue';
import PageHead from './PageHead.vue';
import WorkflowPanel from './WorkflowPanel.vue';
import StandingBadge from './StandingBadge.vue';
import StatusChip from './StatusChip.vue';
import Tip from './Tip.vue';
import { TabsTrigger } from '@/components/ui/tabs';

// The open chain's page: its header (PageHead: what the chain is, where it stands, what the owner can do to it, its tabs;
// the Landings tab lists what it lands) and the tab the address names.
const props = defineProps<{ view: ChainView }>();
const route = useRoute();
const router = useRouter();
const go = (t: string) => router.push({ name: 'chain', params: { id: route.params.id, tab: t }, query: route.query });
const chain = computed(() => props.view.chain.chain);
// While another chain loads the address already names it and its tab: this page keeps its own tab until replaced.
const tab = ref(defaultTab);
watchEffect(() => {
  if (route.params.id === chain.value.id) tab.value = (route.params.tab as string) || defaultTab;
});
// Verification shows once an agent has judged the chain or the goal judge is at work: the server's chip says so.
const standing = computed(() => standingOf(chain.value.id));
const judged = computed(() => !!standing.value.verification);
// A chain that needs the owner, watches a signal or has finished says so in the server's words (Needs you, Watching, Done,
// Not done · not proven: …), never its bare status, which would call a running chain whose agent asks him Running; any other
// says its status. One that needs him says what waits beside it: the kind of its first wait and what it asks (ADR-0078).
const wait = computed(() => standing.value.wait);
const closing = computed(() => standing.value.tone === 'watching' || chain.value.status === 'done');
const state = computed(() => standing.value.text.split(' · ')[0]);
// A paused chain says who paused it and when, and why when he gave one, from its newest pause decision (ADR-0080).
const stream = useStream();
const pausedBy = computed(() => {
  if (chain.value.status !== 'paused') return null;
  const d = [...props.view.decisions].reverse().find((x) => x.decision.kind === 'pause')?.decision;
  if (!d) return null;
  const who = !d.by ? '' : d.by.kind === 'owner' ? ' by you' : ` by ${actorName(d.by, stream.nameOf)}`;
  return { text: `Paused${who} · ${clockAt(d.createdAt)}`, why: d.why ?? '' };
});
// A queued chain has no agents yet: its Chair is hired when it starts.
const working = computed(() => chain.value.status !== 'queued');
// A queued chain's place in the queue.
const detail = computed(() => {
  const c = chain.value;
  return c.status === 'queued' ? '#' + standingOf(c.id).place : '';
});
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col">
    <PageHead :title="chain.title" :tab="tab" @update:tab="go">
      <template #standing>
        <template v-if="wait">
          <ChainStandingBadge :standing="standing" class="flex-none" data-closing>{{ state }}</ChainStandingBadge>
          <Tip :text="`${wait.label}: ${wait.ask}`" when-clipped><span class="min-w-0 max-w-[60ch] truncate text-[12.5px] text-text2" data-wait-ask><b class="font-semibold text-text">{{ wait.label }}:</b> {{ wait.ask }}</span></Tip>
        </template>
        <ChainStandingBadge v-else-if="closing" :standing="standing" data-closing />
        <StatusChip v-else :status="chain.status" :detail="detail" />
        <Tip v-if="pausedBy" :text="pausedBy.why ? `${pausedBy.text} · ${pausedBy.why}` : pausedBy.text" when-clipped><span class="min-w-0 max-w-[60ch] truncate text-[12.5px] text-text2" data-paused-by>{{ pausedBy.text }}<template v-if="pausedBy.why"> · {{ pausedBy.why }}</template></span></Tip>
        <CheckBackChip v-if="view.chain.checkBack" :check-back="view.chain.checkBack" :waiting="chain.status === 'waiting'" />
        <StandingBadge v-if="chain.archivedAt" variant="quiet" icon="archive">Archived</StandingBadge>
      </template>
      <template #end><ChainActions :chain="chain" /></template>
      <template #tabs>
        <template v-if="working">
          <TabsTrigger value="live">Live</TabsTrigger>
          <TabsTrigger value="tree">Agent tree</TabsTrigger>
        </template>
        <TabsTrigger v-if="view.workflows.length" value="workflows">Workflows</TabsTrigger>
        <TabsTrigger v-if="view.landings.length" value="landings">Landings</TabsTrigger>
        <TabsTrigger v-if="judged" value="verification">Verification</TabsTrigger>
        <TabsTrigger value="overview">Overview</TabsTrigger>
      </template>
    </PageHead>
    <div v-if="chain.eventId" class="flex flex-none border-b border-line px-3.5">
      <RouterLink class="inline-flex min-h-6 items-center text-xs text-blue-text hover:underline" :to="{ name: 'events', params: { id: chain.eventId } }" data-event-link>Source event</RouterLink>
    </div>

    <AgentTree v-if="tab === 'tree' && working" :view="view" />
    <LiveView v-if="tab === 'live' && working" :view="view" />
    <WorkflowPanel v-if="tab === 'workflows'" :view="view" />
    <PageBody v-if="tab === 'landings'"><ChainLandings :view="view" /></PageBody>
    <PageBody v-if="tab === 'verification'"><ChainVerification :view="view" /></PageBody>
    <ChainOverview v-if="tab === 'overview'" :view="view" />
  </div>
</template>
