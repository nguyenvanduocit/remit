<script setup lang="ts">
import type { RouteLocationRaw } from 'vue-router';
import { isPattern } from '../pattern';
import { clockAt, timed } from '../ui';
import type { SubscriptionLine } from '../core/generated/types.generated';
import { HOLD_WORDS } from '../subscriptionWords';
import ItemCard from './ItemCard.vue';
import SubscriptionBadge from './SubscriptionBadge.vue';
import { kindSaidOf } from '../subscriptionColumns';
import SubscriptionKindIcon from './SubscriptionKindIcon.vue';

// One subscription on the Signals board, the card every board's items wear (ItemCard): the icon of its kind, its title across
// the card, and under it where it stands or how far off its time is (SubscriptionBadge), then its kind in a word with the time
// it fires at (a routine's next run, a watch's deadline) (an event subscription: whether it holds for every event or for those its condition matches, and the pattern when it listens to more than one signal, ADR-0049; a paused one: the time it keeps). What it does and when it last fired
// are on its page. Then, on a line of its own, why it does not fire though it would: the firing it is held on (the error is on its
// page), else what the delivery of its next firing waits on. `on` marks the one open by its surface. The column it sits in spaces the cards.
defineProps<{ line: SubscriptionLine; to: RouteLocationRaw; on: boolean }>();
// What the time a line fires at is, for the kinds whose time is not plain: a watch's deadline, a routine's next run.
const AT_WORD: Partial<Record<SubscriptionLine['kind'], string>> = { watch: 'deadline ', routine: 'next run ' };
</script>

<template>
  <ItemCard :to="to" :on="on" :title="line.title">
    <template #chip><SubscriptionKindIcon :kind="line.kind" /></template>
    <template #badge><SubscriptionBadge :line="line" /></template>
    <template #meta>
      <span class="min-w-0 [overflow-wrap:anywhere]" data-line>
        {{ kindSaidOf(line.kind)?.noun }}
        <template v-if="line.kind === 'event'">· <span data-when>{{ line.condition ? 'on a match' : 'every event' }}</span><template v-if="line.key && isPattern(line.key)"> · <span class="font-mono text-[12px]" data-pattern>{{ line.key }}</span></template></template>
        <template v-else-if="line.state === 'paused' && timed(line.at)">· <span data-at>planned {{ clockAt(line.at) }}</span></template>
        <template v-else-if="timed(line.at) && line.state !== 'over' && line.state !== 'underway'">· <span data-at>{{ AT_WORD[line.kind] ?? '' }}{{ clockAt(line.at) }}</span></template>
      </span>
      <span v-if="line.hold" class="basis-full [overflow-wrap:anywhere]" data-held-on>{{ HOLD_WORDS[line.hold.reason] }} · <span class="font-mono text-[11px]">{{ line.hold.firing.key }}</span> rev {{ line.hold.firing.rev }}</span>
      <span v-else-if="line.waiting" class="line-clamp-2 basis-full [overflow-wrap:anywhere]" data-waits>Waits: {{ line.waiting }}</span>
    </template>
  </ItemCard>
</template>
