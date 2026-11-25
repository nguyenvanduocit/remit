<script setup lang="ts">
// The column on the left of a view: an optional head (a search) over a list of what to open, which scrolls.
// It fills the resizable panel it sits in (panels.ts sizes it); the panel's handle is the line beside it. The head and the
// list share one inset from the column's edge, so a card and the search above it line up. The list spaces what it holds (a row
// sets no margin of its own).
</script>

<template>
  <aside class="flex min-w-0 flex-1 flex-col bg-bg2">
    <div v-if="$slots.head" class="px-1.5 pt-3 pb-2"><slot name="head" /></div>
    <div class="overflow-auto px-1.5 pb-[18px]" :class="{ 'pt-3': !$slots.head }"><div class="flex flex-col gap-1" data-rows><slot /></div></div>
  </aside>
</template>
