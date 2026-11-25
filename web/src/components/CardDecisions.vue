<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import Icon from './Icon.vue';
import ActorTag from './ActorTag.vue';
import EmptyState from './EmptyState.vue';
import FeedOverturn from './FeedOverturn.vue';
import Markdown from './Markdown.vue';
import ShowMore from './ShowMore.vue';
import { decisionIcon, decisionRows } from '../decisionRows';
import { clockAt } from '../ui';
import { useStream } from '../streamContext';
import type { Decision } from '../core/generated/types.generated';

// What happened in the chain and why, newest first, each with who did it (none on decisions from before it was recorded),
// a page at a time: a long chain decides thousands of times. A decision the owner overturned carries his overturn under it, in
// words beside an icon, instead of a line of its own; a duel judge's pick that stands offers Overturn, where he says why the
// other report wins.
const props = defineProps<{ decisions: readonly Decision[] }>();
const stream = useStream();
const PAGE = 50;
const shown = ref(PAGE);
const drawn = computed(() => decisionRows(props.decisions));
const overturnOf = computed(() => drawn.value.overturnOf);
const newestFirst = computed(() => drawn.value.rows.slice(0, shown.value));
const older = computed(() => drawn.value.rows.length - newestFirst.value.length);
// Another chain's decisions: its page starts over.
watch(() => props.decisions[0]?.chainId, () => (shown.value = PAGE));
</script>

<template>
  <div v-for="d in newestFirst" :key="d.id" class="decision" data-decision :data-kind="d.kind" :data-overturned="overturnOf.has(d.id) ? '' : undefined">
    <div class="ic"><Icon :name="decisionIcon[d.kind] ?? 'dot'" :size="12" /></div>
    <div class="min-w-0">
      <div :class="{ struck: overturnOf.has(d.id) }" data-text><time class="mono mr-1.5 text-faint" :datetime="d.createdAt">{{ clockAt(d.createdAt) }}</time><ActorTag v-if="d.by" class="mr-1.5" :by="d.by" :name-of="stream.nameOf" />{{ d.text }}</div>
      <Markdown v-if="d.why" class="why" :text="d.why" />
      <div v-if="overturnOf.get(d.id)" class="overturn" data-overturn>
        <Icon class="flex-none" name="interrupt" :size="12" />
        <span class="min-w-0 [overflow-wrap:anywhere]"><b class="font-semibold">You overturned it:</b> {{ overturnOf.get(d.id)?.why }}</span>
        <time class="mono ml-auto flex-none" :datetime="overturnOf.get(d.id)?.createdAt">{{ clockAt(overturnOf.get(d.id)?.createdAt ?? '') }}</time>
      </div>
      <FeedOverturn v-else-if="d.kind === 'duel'" :decision="d" ask="Why does the other report win?" />
    </div>
  </div>
  <ShowMore v-if="older" class="mt-2.5" @click="shown += PAGE">Show {{ Math.min(PAGE, older) }} older of {{ older }}</ShowMore>
  <EmptyState v-if="!decisions.length">No decision yet: what the Chair decides, and the Assistant for you, shows here as the chain runs.</EmptyState>
</template>

<style scoped>
.decision {
  display: grid;
  grid-template-columns: 18px 1fr;
  gap: 8px;
  padding: 8px 0;
  border-bottom: 1px dashed var(--rule);
  font-size: 12.5px;
}
.ic {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  margin-top: 1px;
  border-radius: 5px;
  background: var(--violet-bg);
  color: var(--violet-text);
}
.why {
  margin-top: 2px;
  font-size: 11.5px;
  color: var(--mut);
}
.struck {
  color: var(--text2);
  text-decoration: line-through;
  text-decoration-color: var(--mut);
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
  font-size: 11.5px;
}
</style>
