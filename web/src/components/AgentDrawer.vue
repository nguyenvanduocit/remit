<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { useQueryRef } from '../nav';
import { useStickToBottom } from '../useStickToBottom';
import type { AgentStanding, Card, Decision, Delivery, EventRead, InboxItem, Judge } from '../core/generated/types.generated';
import type { Call } from '../core/client';
import type { Freshness } from '../core/generated/replica/types.generated';
import AgentMemory from './AgentMemory.vue';
import AgentPlan from './AgentPlan.vue';
import CardActivity from './CardActivity.vue';
import CardComposer from './CardComposer.vue';
import CardDecisions from './CardDecisions.vue';
import CardHeader from './CardHeader.vue';
import CardSession from './CardSession.vue';
import CardTelemetry from './CardTelemetry.vue';
import ContextDetails from './ContextDetails.vue';
import EmptyState from './EmptyState.vue';
import InboxCallout from './InboxCallout.vue';
import JumpToLatest from './JumpToLatest.vue';
import ModelStatus from './ModelStatus.vue';
import PendingSends from './PendingSends.vue';
import { pendingIn } from '../drafts';

// An agent as the owner opens it: who it is, the panel the address names (`panelKey`, its activity unless named) and the
// box to message it (`deliver` sends, CardComposer's unless given; `composer` false when another box already takes his words,
// as the Assistant's column does while it is open beside this drawer). Questions fold into the panel. Other requests stay pinned
// under the panel whichever panel is open, so Allow and Reject never depend on how far the stream is scrolled; the stream keeps
// a record of where it was asked. The header's "Needs you" takes the keyboard to those answers. The context meter opens the
// Session panel at what fills the context. Whose agent it is (a chain's, the Assistant) is the caller's: it hands over the
// agent's stream, standing and what waits with it, and puts its own buttons in `actions`; `earlier`, when the stream goes back
// further than the events handed over, reads the page before them, from the Activity's one bar for older activity. A caller
// whose agent's decisions are its own kind of record puts them in the `decisions` slot (the Assistant's feed), counting what
// is new in them on the tab (`decisionsNew`). An agent with a memory (`remembers`) shows it in the Memory panel.
type Deliver = (draftId: string, delivery: Delivery) => Call;
const props = withDefaults(
  defineProps<{
    card: Card;
    name: string;
    chair: boolean;
    agent: AgentStanding;
    events: readonly EventRead[];
    waiting: readonly InboxItem[];
    judged: readonly Judge[];
    decisions: readonly Decision[];
    panelKey: string;
    deliver?: Deliver;
    composer?: boolean;
    earlier?: () => void;
    decisionsNew?: number;
    status?: Freshness; // how current its stream is
  }>(),
  { composer: true },
);
const tab = useQueryRef(props.panelKey, 'activity', ['activity', 'decisions', 'plan', 'session', 'memory'] as const);
const revealContext = ref(false);
function goToContext() {
  tab.value = 'session';
  revealContext.value = true;
}

// The body follows the newest activity while the owner is at its bottom, and stays put while they read older blocks, a
// button counting what arrived meanwhile and taking him back (JumpToLatest). The
// panels share it, and only the activity is a feed: each panel opens at its start, the activity at its newest line (what
// waits on the owner, when something does) and any other at its top.
const body = ref<HTMLElement | null>(null);
// The owner's messages on their way join the end of the stream, where the server's copy will stand, not the box: the box
// keeps its place while they come and go. One the server took is its copy in the stream from then on, so it is not shown twice.
const onTheirWay = pendingIn(() => ({ home: props.card.home, cardId: props.card.id }));
const pending = computed(() => onTheirWay.value.filter((p) => p.status !== 'sent'));
const feed = computed(() => (tab.value === 'activity' ? body.value : null));
const { stuck, unseen, onScroll, follow } = useStickToBottom(
  feed,
  () => props.events.at(-1)?.event.id + ':' + (props.events.at(-1)?.event.text.length ?? 0) + ':' + props.waiting.length + ':' + pending.value.map((p) => p.status).join(),
  () => props.events,
);
watch(tab, (t) => {
  if (t === 'activity') void follow();
  else if (body.value) body.value.scrollTop = 0;
});
const questions = computed(() => props.waiting.filter((item) => item.kind === 'question'));
const otherRequests = computed(() => props.waiting.filter((item) => item.kind !== 'question'));
const questionList = ref<HTMLElement | null>(null);
const pinned = ref<HTMLElement | null>(null);
function goToWaiting() {
  const action = pinned.value?.querySelector<HTMLElement>('button, input');
  if (action) return action.focus();
  const question = questionList.value?.querySelector('details');
  if (question) {
    question.open = true;
    question.querySelector('summary')?.focus();
    question.scrollIntoView({ block: 'start', inline: 'nearest' });
  }
}
watch(() => props.card.id, () => void follow(), { immediate: true });
</script>

<template>
  <aside class="flex min-h-0 min-w-0 flex-1 flex-col bg-bg2" :aria-label="name">
    <CardHeader v-model:tab="tab" :card="card" :chair="chair" :agent="agent" :decided="chair || decisions.length > 0 || !!$slots.decisions" :decisions-new="decisionsNew" @context="goToContext" @needs="goToWaiting"><template v-if="$slots.title" #title><slot name="title" /></template><template #actions><slot name="actions" /></template></CardHeader>
    <ModelStatus v-if="status && tab === 'activity'" :status="status" />
    <div class="relative flex min-h-0 flex-1 flex-col">
      <div ref="body" class="flex-1 overflow-auto px-5 py-4 [contain:content]" @scroll="onScroll">
        <template v-if="tab === 'decisions'"><slot name="decisions"><CardDecisions :decisions="decisions" /></slot></template>
        <CardActivity v-if="tab === 'activity'" :key="card.id" :card="card" :chair="chair" :events="events" :agent="agent" :waiting="waiting" :judged="judged" :earlier="earlier" />
        <PendingSends v-if="tab === 'activity' && composer" class="mt-3" :pending="pending" />
        <template v-if="tab === 'plan'">
          <AgentPlan v-if="card.plan?.length" :plan="card.plan" :title="card.deliverable" />
          <EmptyState v-else>No plan yet.</EmptyState>
        </template>
        <template v-if="tab === 'session'">
          <CardSession :card="card" />
          <ContextDetails v-model:reveal="revealContext" class="mt-4" :card="card" />
          <CardTelemetry :card="card" />
        </template>
        <template v-if="tab === 'memory'">
          <AgentMemory v-if="agent.remembers" :card-id="card.id" :name="name" :view-key="`${panelKey}Memory`" :employee="card.employee" />
          <EmptyState v-else>{{ name }} keeps no memory: it is no employee's.</EmptyState>
        </template>
        <div v-if="questions.length" ref="questionList" class="mt-3 grid gap-2">
          <details v-for="item in questions" :key="item.id" class="min-w-0 rounded-md bg-panel">
            <summary class="cursor-pointer rounded-md px-3 py-2 text-sm font-semibold text-text">
              <span class="inline-block max-w-[90%] truncate align-middle">{{ item.title }}</span>
            </summary>
            <InboxCallout :item="item" />
          </details>
        </div>
      </div>
      <JumpToLatest v-if="tab === 'activity' && !stuck" class="absolute right-4 bottom-3" :unseen="unseen" @click="follow" />
    </div>
    <div v-if="otherRequests.length" ref="pinned" class="flex max-h-[45%] flex-none flex-col gap-2 overflow-auto border-t border-line px-3 py-2.5" data-pinned-asks>
      <InboxCallout v-for="item in otherRequests" :key="item.id" :item="item" />
    </div>
    <CardComposer v-if="composer" :card="card" :name="name" :chair="chair" :agent="agent" :deliver="deliver" @sent="follow">
      <template v-if="$slots.composerOptions" #options><slot name="composerOptions" /></template>
    </CardComposer>
  </aside>
</template>
