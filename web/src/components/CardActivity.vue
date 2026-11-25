<script setup lang="ts">
import { computed } from 'vue';
import { blocks, toolOf } from '../stream';
import { useNewest } from '../useNewest';
import { agentName, doingText } from '../ui';
import type { AgentStanding, Card, EventRead, InboxItem, Judge } from '../core/generated/types.generated';
import Markdown from './Markdown.vue';
import StreamBlocks from './StreamBlocks.vue';
import Callout from './Callout.vue';
import EmptyState from './EmptyState.vue';
import LiveDot from './LiveDot.vue';
import ShowMore from './ShowMore.vue';

// The agent's stream: why it is here, what it read, thought, ran and said, what it asked of the owner (`waiting`, with its
// answers) where it asked, each verdict it gave where it gave it (`judged`: a judge's work is its verdict), a typing line
// while it works, and last, the error it stopped on. The typing line says what the server says it is doing (`agent`).
// One bar above it shows older activity: first what is loaded but not drawn, then, once all of that shows, the page before
// it from the server (`earlier`, when the stream goes back further than `events`).
const props = defineProps<{ card: Card; chair: boolean; events: readonly EventRead[]; agent: AgentStanding; waiting: readonly InboxItem[]; judged: readonly Judge[]; earlier?: () => void }>();
// Each round of a judge is a block of its own, placed by the time it was given.
const rounds = computed(() => props.judged.flatMap((j) => [j.latest, ...j.earlier].map((v) => ({ ...j, latest: v, earlier: [] }))));
const { shown, older, more } = useNewest(() => blocks(props.events, props.waiting, rounds.value));
const showOlder = () => (older.value ? more() : void props.earlier?.());
const busy = computed(() => props.card.status === 'running' || props.card.status === 'permission');
// The feed already shows a running tool (its card, with a spinner) and a thought being written (its Thinking block, open),
// and an ask it ends on says what the agent waits for, so the typing line is only for the rest.
const feedShowsIt = computed(() => {
  const last = props.events[props.events.length - 1];
  if (last?.event.kind === 'tool') return toolOf(last).status === 'running';
  return last?.event.kind === 'thought' && busy.value;
});
const endsOnAsk = computed(() => shown.value[shown.value.length - 1]?.type === 'ask');
// A failed turn the owner can retry is an item of `waiting`, which says the error itself.
const failure = computed(() => props.waiting.some((i) => i.kind === 'error'));
</script>

<template>
  <Callout v-if="!chair && card.why" tone="note" class="mb-2.5"><template #title>Why this agent is here</template><Markdown type="quiet" :text="card.why" /></Callout>
  <ShowMore v-if="older || earlier" class="mb-2.5" @click="showOlder">{{ older ? `Show older (${older})` : 'Show older activity' }}</ShowMore>
  <StreamBlocks :blocks="shown" :live="busy" />
  <div v-if="(busy || card.status === 'starting') && !feedShowsIt && !endsOnAsk" class="typing" :class="doingText(agent.doingTone) || 'text-mut'"><LiveDot :live="agent.doingTone === 'busy'" />{{ agent.doing }}</div>
  <EmptyState v-if="!shown.length">No activity yet: what the agent does shows here as it works.</EmptyState>
  <Callout v-if="card.error && !failure" tone="error" class="mt-3">
    <template #title>{{ agentName(card) }} error</template>
    <pre>{{ card.error }}</pre>
  </Callout>
</template>

<style scoped>
/* The agent is at work: its dot pings beside what it is doing, in the colour of that, as in its header. */
.typing {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 10px 0 4px;
  font-size: 12px;
}
</style>
