<script setup lang="ts">
import { computed, ref } from 'vue';
import { useRoute } from 'vue-router';
import { useQueryRef } from '../nav';
import type { ChainView } from '../core/generated/types.generated';
import EmptyState from './EmptyState.vue';
import LandingCard from './LandingCard.vue';
import LandingDetail from './LandingDetail.vue';
import LandingFiles from './LandingFiles.vue';
import ShowMore from './ShowMore.vue';

// What the chain lands (ADR-0114), live from the replica's landings: a card per version, newest first, then the one picked
// (?landing=, else the newest) as its record and its files. No landing waits for the owner: Remit lands each once its gates
// approve it. What he wants changed he tells the Chair on this screen.
const props = defineProps<{ view: ChainView }>();
const route = useRoute();
const newestFirst = computed(() => [...props.view.landings].reverse());
const picked = useQueryRef<string>('landing', '');
const current = computed(() => newestFirst.value.find((l) => l.landing.id === picked.value) ?? newestFirst.value[0]);
// The cards a page at a time (a chain across many repositories lands many versions), the open one always among them.
const PAGE = 12;
const shown = ref(PAGE);
const cards = computed(() => {
  const page = newestFirst.value.slice(0, shown.value);
  return current.value && !page.includes(current.value) ? [...page, current.value] : page;
});
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col gap-5">
    <EmptyState v-if="!current">Nothing landed yet: the Chair lands the chain’s work, and each version shows here as its gates judge it.</EmptyState>
    <template v-else>
      <div v-if="newestFirst.length > 1">
        <ul class="m-0 grid list-none gap-2.5 p-0 [grid-template-columns:repeat(auto-fill,minmax(280px,1fr))]" aria-label="The chain’s landings" data-landings>
          <li v-for="{ landing: l } in cards" :key="l.id" class="min-w-0"><LandingCard :landing="l" :where="`${l.repo} · version ${l.version.number}`" :to="{ query: { ...route.query, landing: l.id, file: undefined } }" :on="l.id === current.landing.id" /></li>
        </ul>
        <ShowMore v-if="newestFirst.length > shown" class="mt-2.5" @click="shown += PAGE">Show {{ Math.min(PAGE, newestFirst.length - shown) }} more of {{ newestFirst.length - shown }} landings</ShowMore>
      </div>
      <LandingDetail :landing="current.landing" :release="current.release" :chain="view.landings" />
      <LandingFiles :landing="current.landing" />
    </template>
  </div>
</template>
