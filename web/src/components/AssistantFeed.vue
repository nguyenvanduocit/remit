<script setup lang="ts">
import { computed, ref, type ComponentPublicInstance } from 'vue';
import { assistant } from '../assistant';
import { useSeenMark } from '../seen';
import { clockAt, fullTime } from '../ui';
import Ago from './Ago.vue';
import EmptyState from './EmptyState.vue';
import FeedDissent from './FeedDissent.vue';
import FeedOverturn from './FeedOverturn.vue';
import Icon from './Icon.vue';
import Markdown from './Markdown.vue';
import Tip from './Tip.vue';
import Until from './Until.vue';

// What the Assistant decided for the owner (ADR-0062), newest first as the server lays it out: each line says what it decided
// or left to him, why, and when. A line he overturned says so with his word, in words beside an icon; an answer it gave for him
// that stands offers Overturn, where he says what he decides instead; a second judge's dissent from that answer shows above it
// (FeedDissent, ADR-0103). An answer that cannot be undone waits out its overturn window (ADR-0101): the line says when it
// takes effect, counting down, above Overturn. What came since he last looked is marked seen once he has
// viewed it (useSeenMark, ADR-0079): the newest line in sight for a second on a visible, focused page.
const feed = computed(() => assistant.feed);
const newest = ref<HTMLElement | null>(null);
const holdNewest = (el: Element | ComponentPublicInstance | null, i: number) => i === 0 && (newest.value = el instanceof HTMLElement ? el : null);
useSeenMark(() => (feed.value?.unseen ? { kind: 'assistant_feed' } : undefined), newest);
</script>

<template>
  <ol v-if="feed?.lines.length" class="m-0 flex list-none flex-col p-0" aria-label="What the Assistant decided for you" data-rows>
    <li v-for="(l, i) in feed.lines" :key="l.decision.id" :ref="(el) => holdNewest(el, i)" class="line" :class="{ over: !!l.overturn }" :data-feed-line="l.decision.id" data-row :data-overturned="l.overturn ? '' : undefined">
      <span class="ic" :class="{ left: !l.answered }" aria-hidden="true"><Icon :name="l.answered ? 'spark' : 'help'" :size="12" /></span>
      <div class="min-w-0">
        <div class="flex min-w-0 items-baseline gap-2">
          <p class="m-0 min-w-0 flex-1 text-text [overflow-wrap:anywhere]" :class="{ struck: !!l.overturn }" data-text>{{ l.decision.text }}</p>
          <Tip :text="fullTime(l.decision.createdAt)"><time class="flex-none rounded-sm text-[11px] text-mut" :datetime="l.decision.createdAt"><Ago :at="l.decision.createdAt" /></time></Tip>
        </div>
        <Markdown v-if="l.decision.why" type="quiet" class="why" :text="l.decision.why" data-why />
        <FeedDissent v-if="l.dissent" :dissent="l.dissent" />
        <div v-if="l.overturn" class="overturn" data-overturn>
          <Icon name="interrupt" :size="12" />
          <span class="min-w-0 [overflow-wrap:anywhere]"><b class="font-semibold">You overturned it:</b> {{ l.overturn.why }}</span>
          <Tip :text="fullTime(l.overturn.createdAt)"><time class="ml-auto flex-none rounded-sm text-[11px] text-mut" :datetime="l.overturn.createdAt"><Ago :at="l.overturn.createdAt" /></time></Tip>
        </div>
        <template v-else-if="l.answered">
          <p v-if="l.takesEffectAt" class="window" data-takes-effect>
            <Icon name="clock" :size="12" />
            <span class="min-w-0">Takes effect at {{ clockAt(l.takesEffectAt) }}, <Until :at="l.takesEffectAt" />, unless you overturn it</span>
          </p>
          <FeedOverturn :decision="l.decision" />
        </template>
      </div>
    </li>
  </ol>
  <EmptyState v-else>The Assistant has decided nothing for you yet. What it answers for you, or leaves to you, shows here.</EmptyState>
</template>

<style scoped>
.line {
  display: grid;
  grid-template-columns: 20px 1fr;
  gap: 8px;
  padding: 10px 0;
  border-bottom: 1px dashed var(--rule);
  font-size: 12.5px;
}
.ic {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border-radius: 5px;
  background: var(--violet-bg);
  color: var(--violet-text);
}
.ic.left {
  background: var(--panel3);
  color: var(--text2);
}
.struck {
  color: var(--text2);
  text-decoration: line-through;
  text-decoration-color: var(--mut);
}
.why {
  margin-top: 2px;
  font-size: 11.5px;
}
.window {
  display: flex;
  align-items: baseline;
  gap: 6px;
  margin: 6px 0 0;
  color: var(--sand);
  font-size: 11.5px;
}
.overturn {
  display: flex;
  align-items: baseline;
  gap: 6px;
  margin-top: 6px;
  padding: 6px 8px;
  border-radius: 6px;
  background: var(--orange-bg);
  color: var(--orange-text);
}
</style>
