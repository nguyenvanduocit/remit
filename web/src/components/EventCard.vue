<script setup lang="ts">
import type { RouteLocationRaw } from 'vue-router';
import { VIA } from '../events';
import type { EventRecord } from '../core/generated/types.generated';
import Ago from './Ago.vue';
import { actionIcon } from './FiringItem.vue';
import Icon from './Icon.vue';
import IconChip from './IconChip.vue';
import ItemCard from './ItemCard.vue';

// One event on the Events list, the card every list's items wear (ItemCard): the chip of the way it came, the signal it came
// through across the card, and under it how many subscriptions acted on it (or that none held for it), then when it came; below
// the link, what the first two did, in the server's words. What arrived is on its page: a list carries none of it. `on` marks the
// one open by its surface.
const props = defineProps<{ event: EventRecord; to: RouteLocationRaw; on: boolean }>();
const SHOWN = 2;
</script>

<template>
  <ItemCard :to="to" :on="on" :title="event.signal" mono :data-event="event.id" :data-fired="event.firings.length">
    <template #chip><IconChip :icon="VIA[event.via].icon" class="bg-panel3 text-mut" :label="VIA[event.via].word" /></template>
    <template #badge>
      <span v-if="event.firings.length" class="text-text2" data-firings>{{ event.firings.length === 1 ? '1 firing' : `${event.firings.length} firings` }}</span>
      <span v-else class="text-mut" data-none>No subscription held for it</span>
    </template>
    <template #meta><Ago :at="event.at" /></template>
    <template v-if="event.firings.length" #below>
      <ul class="m-0 mt-1.5 flex list-none flex-col gap-1 p-0 pl-9 text-[11.5px] leading-[1.4] text-text2">
        <li v-for="(f, i) in props.event.firings.slice(0, SHOWN)" :key="i" class="flex min-w-0 items-baseline gap-1.5" data-outcome><Icon class="flex-none self-center text-mut" :name="actionIcon(f.action)" :size="11" /><span class="line-clamp-1 min-w-0 [overflow-wrap:anywhere]">{{ f.outcome }}</span></li>
        <li v-if="event.firings.length > SHOWN" class="text-mut">and {{ event.firings.length - SHOWN }} more</li>
      </ul>
    </template>
  </ItemCard>
</template>
