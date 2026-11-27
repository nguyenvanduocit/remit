<script setup lang="ts">
import type { RouteLocationRaw } from 'vue-router';
import { landingAt, PRIORITY_SAYS } from '../landings';
import type { Landing } from '../core/generated/types.generated';
import Ago from './Ago.vue';
import IconChip from './IconChip.vue';
import ItemCard from './ItemCard.vue';
import LandingBadge from './LandingBadge.vue';

// One landing as a card: its title, where it stands, `where` (its repository and version on a chain's screen, its chain in a
// queue), how soon its Chair says to read it, and since when it stands so. A revert says so by its chip and its word. The
// card opens the landing on its chain's screen.
defineProps<{ landing: Landing; where: string; to: RouteLocationRaw; on: boolean }>();
</script>

<template>
  <ItemCard :to="to" :on="on" :title="landing.version.title" :data-landing="landing.id">
    <template v-if="landing.reverts" #chip><IconChip icon="repeat" class="bg-red-tint text-red" /></template>
    <template #badge><LandingBadge :landing="landing" /></template>
    <template #meta>
      <span v-if="landing.reverts" class="font-medium text-text2">Revert</span>
      <span class="min-w-0 truncate" data-where>{{ where }}</span>
      <span v-if="landing.priority">{{ PRIORITY_SAYS[landing.priority] }}</span>
      <Ago :at="landingAt(landing)" />
    </template>
  </ItemCard>
</template>
