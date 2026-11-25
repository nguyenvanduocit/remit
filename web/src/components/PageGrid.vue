<script setup lang="ts">
// The sections of an item's page, one under another and two across where the page is wide (the page is a container). The
// single column is declared (`grid-cols-1`, a track that may shrink) so a long line inside a section scrolls in its own box
// instead of widening the track and the page.
</script>

<template>
  <div class="grid grid-cols-1 items-start gap-5 @[1100px]:grid-cols-2"><slot /></div>
</template>
