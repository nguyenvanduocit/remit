<script setup lang="ts">
import { computed } from 'vue';
import { catalog } from '../catalog';
import EmptyPage from './EmptyPage.vue';
import PageBody from './PageBody.vue';
import PageTabs from './PageTabs.vue';
import SeatCard from './SeatCard.vue';

// Who is at work now, across every chain and the Assistant: one list of seats (SeatCard), in the server's order (what waits
// on the owner first, then what runs, the longest at it first, then the rest that wait), kept live by its push. Nothing here
// orders, counts or decides what is at work: the server lays it out (crates/orch/src/atwork.rs).
const seats = computed(() => catalog.atWork?.seats ?? []);
</script>

<template>
  <section class="flex min-h-0 flex-1 flex-col overflow-hidden" aria-label="At work">
    <PageTabs group="work" />
    <EmptyPage v-if="!seats.length" title="No agent at work" icon="agent">
      <template #what>Every agent that runs or waits shows here, in every chain and the Assistant: what it works on now, since when, and the tool it uses this minute. You stop one here, with a why its Chair reads.</template>
      <template #how>
        <li>Tell the Assistant what you want done; it starts a chain, and the Chair hires the agents.</li>
        <li>Each agent shows here while it runs or waits, and leaves when its card is done.</li>
      </template>
    </EmptyPage>
    <PageBody v-else>
      <div class="mx-auto w-full max-w-3xl">
        <ul class="m-0 flex list-none flex-col gap-stack p-0" aria-label="Seats at work">
          <li v-for="s in seats" :key="s.cardId"><SeatCard :seat="s" /></li>
        </ul>
      </div>
    </PageBody>
  </section>
</template>
