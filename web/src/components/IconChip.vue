<script setup lang="ts">
import Icon from './Icon.vue';
import Spinner from './Spinner.vue';

// The chip that says what kind of thing something is or where it stands, as a shape on a tint (never by colour
// alone), small enough to stand in a line of text, as a card's second line starts with it: the icon, or the ring while it is at work. The caller gives the tint and the colour as tokens (`bg-blue-bg
// text-blue-text`). `label` is what a screen reader says in its place; without one the words beside it say it.
defineProps<{ icon: string; label?: string; working?: boolean }>();
</script>

<template>
  <span class="chip" :role="label ? 'img' : undefined" :aria-label="label" :aria-hidden="label ? undefined : 'true'" data-chip><Spinner v-if="working" /><Icon v-else :name="icon" :size="12" /></span>
</template>

<style scoped>
.chip {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border-radius: 6px;
}
</style>
