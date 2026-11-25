<script setup lang="ts">
import { computed } from 'vue';
import { assistant } from '../assistant';
import { fullTime } from '../ui';
import type { InboxItem } from '../core/generated/types.generated';
import ActorTag from './ActorTag.vue';
import Ago from './Ago.vue';
import FeedDissent from './FeedDissent.vue';
import FeedOverturn from './FeedOverturn.vue';
import Icon from './Icon.vue';
import Tip from './Tip.vue';

// Who decided an item that no longer waits, and what, in one line (ADR-0087): a permission's own line in the server's words
// ("Allowed by the Assistant, for you"), else who (the owner, the Assistant for him, Remit) and the answer; then when, why
// (the Assistant's line of its feed about this item, a hint on "why") and Overturn while its answer stands; a second judge's
// dissent from the answer (ADR-0103) shows under it. The owner's
// overturn, once he made one, is his record under it. An item withdrawn with no one deciding says so; one answered before
// the server recorded who answered it says only what.
const props = defineProps<{ item: InboxItem }>();
const by = computed(() => props.item.answeredBy);
const said = computed(() => props.item.permission?.decided);
const line = computed(() => (by.value?.kind === 'assistant' ? assistant.feed?.lines.find((l) => l.decision.item === props.item.id) : undefined));
// The server's answer names what an agent decided; its tag says only that an agent did.
const anAgent = () => 'An agent';
</script>

<template>
  <div class="flex flex-col gap-1" data-decided :data-by="by?.kind">
    <div class="flex min-w-0 flex-wrap items-center gap-x-1.5 gap-y-1 text-[12.5px] text-text">
      <span v-if="said" class="min-w-0 [overflow-wrap:anywhere]" data-said>{{ said }}</span>
      <template v-else-if="by">
        <ActorTag :by="by" :name-of="anAgent" />
        <span>decided</span><span v-if="item.answer" class="min-w-0 font-medium [overflow-wrap:anywhere]" data-answer>{{ item.answer }}</span>
      </template>
      <span v-else-if="item.status === 'cancelled'" class="text-text2" data-withdrawn>No one decided it: it no longer waits.</span>
      <template v-else><span>Answered</span><span v-if="item.answer" class="min-w-0 font-medium [overflow-wrap:anywhere]" data-answer>{{ item.answer }}</span></template>
      <template v-if="item.answeredAt">
        <span class="text-mut" aria-hidden="true">·</span>
        <Tip :text="fullTime(item.answeredAt)"><time class="rounded-sm text-mut" :datetime="item.answeredAt" data-when><Ago :at="item.answeredAt" /></time></Tip>
      </template>
      <template v-if="line?.decision.why">
        <span class="text-mut" aria-hidden="true">·</span>
        <Tip :text="line.decision.why"><span class="why rounded-sm text-text2" data-why>why</span></Tip>
      </template>
      <FeedOverturn v-if="line?.answered && !line.overturn" class="mt-0 [&[data-overturn-form]]:basis-full" :decision="line.decision" />
    </div>
    <FeedDissent v-if="line?.dissent" :dissent="line.dissent" />
    <div v-if="line?.overturn" class="overturn flex items-baseline gap-1.5" data-overturn>
      <Icon name="interrupt" :size="12" />
      <span class="min-w-0 [overflow-wrap:anywhere]"><b class="font-semibold">You overturned it:</b> {{ line.overturn.why }}</span>
    </div>
  </div>
</template>

<style scoped>
/* The owner's overturn is his act, so it is in ink like him (ADR-0090), sunk below the card as a block of record; the words
   "You overturned it" and the icon say what it is. */
.overturn {
  margin-top: 2px;
  padding: 6px 8px;
  border: 1px solid var(--line);
  border-radius: 6px;
  background: var(--bg);
  color: var(--text2);
  font-size: 12px;
}
.why {
  text-decoration: underline dotted;
  text-underline-offset: 3px;
  cursor: help;
}
.overturn b,
.overturn .icon {
  color: var(--text);
}
</style>
