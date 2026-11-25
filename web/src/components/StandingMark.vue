<script setup lang="ts">
import { toneText, type Tone } from '../ui';
import Icon from './Icon.vue';
import Spinner from './Spinner.vue';

// Where one small thing stands, as a mark without words: a tool call, a step of a plan, a note in the feed, a gate. The ring
// while it is at work, else its icon, in its tone's colour (ui.ts). `label` is what a screen reader says in its place.
defineProps<{ tone: Tone; working?: boolean; icon?: string; label?: string }>();
</script>

<template>
  <span class="inline-flex flex-none items-center" :class="toneText[tone]" :role="label ? 'img' : undefined" :aria-label="label" :aria-hidden="label ? undefined : 'true'">
    <Spinner v-if="working" /><Icon v-else-if="icon" :name="icon" :size="12" />
  </span>
</template>
