<script setup lang="ts">
import type { RouteLocationRaw } from 'vue-router';
import { shortSha } from '../landings';
import { releaseName } from '../releases';
import type { Release } from '../core/generated/types.generated';
import Ago from './Ago.vue';
import ItemCard from './ItemCard.vue';
import ReleaseBadge from './ReleaseBadge.vue';

// One release as a card: its version, where it stands, the commit it released and since when it stands so. The card opens
// its record on the repository's Releases page.
defineProps<{ release: Release; to: RouteLocationRaw; on: boolean }>();
</script>

<template>
  <ItemCard :to="to" :on="on" :title="`Release ${releaseName(release)}`" :data-release="release.id">
    <template #badge><ReleaseBadge :state="release.state" /></template>
    <template #meta>
      <span class="mono">{{ shortSha(release.commit) }}</span>
      <Ago :at="release.endedAt ?? release.createdAt" />
    </template>
  </ItemCard>
</template>
