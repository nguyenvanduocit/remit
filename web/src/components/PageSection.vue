<script setup lang="ts">
// One section of an item's page, as a card: a heading (with the `count` of what it holds, and `aside` at the heading's end) over
// its body; a `flush` body (a table) takes the card's whole width. Every section of every page is this one, so a page reads as a few named blocks, not a run of text. `id` is the
// heading's, for the section's aria-labelledby. It clips to its corners without being a scroller of its own (overflow: clip),
// so a sticky part of its body (a memory book's tabs) sticks to the page's scroll.
defineProps<{ title: string; count?: number; id?: string; flush?: boolean }>();
</script>

<template>
  <section class="overflow-clip rounded-lg border border-line bg-panel" :aria-labelledby="id">
    <header class="flex min-h-9 items-center gap-2 border-b border-line px-4 py-1.5">
      <h2 :id="id" class="text-[13px] font-semibold text-text">{{ title }}</h2>
      <span v-if="count !== undefined" class="rounded-full bg-panel3 px-1.5 text-[11px] text-mut tabular-nums">{{ count }}</span>
      <div class="ml-auto flex min-w-0 items-center gap-2 text-[12px] text-mut"><slot name="aside" /></div>
    </header>
    <div :class="flush ? '' : 'px-4 py-3.5'"><slot /></div>
  </section>
</template>
