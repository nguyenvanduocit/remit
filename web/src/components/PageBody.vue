<script setup lang="ts">
// The scrolling body of a page under its header, at the one padding every page has. `feed` leaves room at the bottom for
// what floats over a feed or an outline (↓ Latest), so its last line can scroll clear of it.
defineProps<{ feed?: boolean }>();
</script>

<template>
  <div class="min-h-0 flex-1 overflow-auto px-3.5 pt-3.5" :class="feed ? 'pb-10' : 'pb-3.5'"><slot /></div>
</template>
