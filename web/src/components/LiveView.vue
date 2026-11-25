<script setup lang="ts">
import { computed, ref } from 'vue';
import { agentOf, opened } from '../openChain';
import { useSelected } from '../nav';
import type { ChainView } from '../core/generated/types.generated';
import { feed, segments, type FeedItem, type Segment } from '../stream';
import { agentName, hm } from '../ui';
import { useNewest } from '../useNewest';
import { useSeenMark } from '../seen';
import { useStickToBottom } from '../useStickToBottom';
import AgentBadge from './AgentBadge.vue';
import AgentStandingBadge from './AgentStandingBadge.vue';
import ChainAway from './ChainAway.vue';
import ModelStatus from './ModelStatus.vue';
import EmptyState from './EmptyState.vue';
import OutcomeLine from './OutcomeLine.vue';
import RetrospectiveCard from './RetrospectiveCard.vue';
import StatusChip from './StatusChip.vue';
import JumpToLatest from './JumpToLatest.vue';
import StreamBlocks from './StreamBlocks.vue';
import ShowMore from './ShowMore.vue';

// Every agent of the chain, interleaved, as it happens: each run of one agent's work a card, its badge, its name and when
// the run began over what it did (StreamBlocks, each turn of it framed), and between the runs where they happened, each outcome
// of the chain with who and why (OutcomeLine) and each retrospective the coach wrote (RetrospectiveCard). Over it, as the owner
// returns, what happened while he was away (ChainAway). Sticks to the bottom unless the owner scrolls up to
// read, and then counts what arrived and who wrote it on the way back down (JumpToLatest), so its body is PageBody's (feed) with the element it scrolls held here; only the newest page of runs is drawn, the
// older ones a bar away (useNewest). Its end in sight marks the chain seen (useSeenMark).
const props = defineProps<{ view: ChainView }>();
const el = ref<HTMLElement | null>(null);
const cards = computed(() => new Map(props.view.cards.map((c) => [c.id, c])));
const allSegs = computed<Segment[]>((previous) => segments(opened.events, previous));
const outcomes = computed(() => props.view.decisions.filter((d) => d.outcome).map((d) => d.decision));
const allItems = computed<FeedItem[]>(() => feed(allSegs.value, outcomes.value, props.view.retrospectives));
const { shown: items, older, more } = useNewest(() => allItems.value);
const isLive = (cardId: string, i: number) => {
  const c = cards.value.get(cardId);
  // Only the newest run of a card is still being written.
  const later = items.value.slice(i + 1).some((s) => s.type === 'run' && s.run.cardId === cardId);
  return !later && !!c && (c.status === 'running' || c.status === 'permission');
};
// An agent something waits on the owner with is marked on each of its runs, as wherever else it shows.
const needs = (cardId: string) => {
  const a = agentOf(cardId);
  return a.waiting.length ? a : null;
};
const nameOf = (cardId: string) => {
  const c = cards.value.get(cardId);
  return c ? agentName(c) : '';
};
const selected = useSelected();
const pick = (cardId: string) => {
  const c = cards.value.get(cardId);
  selected.value = c?.origin === 'chair' ? 'chair' : cardId;
};
// The owner has seen the chain up to its newest event once the end of the feed, under its newest event, has been in sight for a
// second (ADR-0079): scrolled up to read older runs, nothing is marked.
const end = ref<HTMLElement | null>(null);
useSeenMark(() => {
  const last = opened.events[opened.events.length - 1];
  return last && { kind: 'home', home: props.view.chain.chain.id, eventId: last.event.id };
}, end);
const { stuck, unseen, onScroll, follow } = useStickToBottom(
  el,
  () => allItems.value.length + ':' + opened.events.at(-1)?.event.id + ':' + (opened.events.at(-1)?.event.text.length ?? 0),
  () => opened.events,
);
// Who wrote what arrived while the owner read older runs: each agent once, in the order they wrote.
const unseenFrom = computed(() => [...new Set(opened.events.slice(opened.events.length - unseen.value).map((e) => nameOf(e.event.cardId)))].join(', '));
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col">
    <ChainAway v-if="view.away" :away="view.away" :chain-id="view.chain.chain.id" />
    <ModelStatus :status="opened.status" />
    <div class="relative flex min-h-0 flex-1">
      <div ref="el" class="feed min-h-0 flex-1 overflow-auto px-4 pt-4 pb-10 [contain:content]" @scroll="onScroll">
        <ShowMore v-if="older" @click="more">Show older ({{ older }})</ShowMore>
        <EmptyState v-if="!items.length">No activity yet.</EmptyState>
        <template v-for="(it, i) in items" :key="it.key">
          <OutcomeLine v-if="it.type === 'outcome'" :decision="it.decision" />
          <RetrospectiveCard v-else-if="it.type === 'retro'" :read="it.retrospective" />
          <article v-else class="run" :class="{ writing: isLive(it.run.cardId, i) }" :data-run="it.run.cardId">
            <header class="flex items-center gap-2 text-xs">
              <button type="button" class="flex min-w-0 items-center gap-2 rounded-md text-left" :aria-label="`Open ${nameOf(it.run.cardId)}`" @click="pick(it.run.cardId)">
                <AgentBadge :agent="cards.get(it.run.cardId)?.origin === 'chair' ? 'chair' : cards.get(it.run.cardId)?.agent ?? '?'" :size="22" />
                <b class="truncate text-[12.5px]">{{ nameOf(it.run.cardId) }}</b>
                <AgentStandingBadge v-if="needs(it.run.cardId)" :agent="needs(it.run.cardId)!" />
                <StatusChip v-else-if="isLive(it.run.cardId, i)" :status="cards.get(it.run.cardId)!.status" />
              </button>
              <time class="ml-auto flex-none text-[11px] text-mut tabular-nums" :datetime="it.run.events[0].event.createdAt">{{ hm(it.run.events[0].event.createdAt) }}</time>
            </header>
            <div class="mt-1.5"><StreamBlocks :blocks="it.run.blocks" :live="isLive(it.run.cardId, i)" compact /></div>
          </article>
        </template>
        <div v-if="items.length" ref="end" class="h-px" aria-hidden="true" data-feed-end></div>
      </div>
      <JumpToLatest v-if="!stuck" class="absolute right-4 bottom-3.5" :unseen="unseen" :who="unseenFrom" @click="follow" />
    </div>
  </div>
</template>

<style scoped>
/* The feed flows as blocks, a margin between runs: a flex column with a gap here costs every wheel step a layout of all
   its runs (measured on 20,000 events: 38 ms a frame against 22). */
.feed > * + * {
  margin-top: 10px;
}
/* One agent's run of the feed, a card; the one being written says so by its surface, lit from the left. */
.run {
  contain: content;
  padding: 10px 14px 8px;
  border-radius: 8px;
  border: 1px solid var(--line);
  background: var(--panel);
}
.run.writing {
  background: linear-gradient(90deg, var(--blue-wash), var(--panel) 60%);
}
</style>
