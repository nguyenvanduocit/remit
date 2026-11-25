<script setup lang="ts">
import { toneText, type Tone } from '../ui';
import Icon from './Icon.vue';

// A line that says why something stands where it stands, with its icon in front: why an event waits, why a run is held,
// what went wrong with a key. Its tone colours both (ui.ts); a warning unless said otherwise.
withDefaults(defineProps<{ tone?: Tone; icon?: string }>(), { tone: 'warning', icon: 'alert' });
</script>

<template>
  <div class="flex items-start gap-1.5 text-[12px] leading-[1.4]" :class="toneText[tone]">
    <Icon class="mt-px flex-none" :name="icon" :size="12" /><span class="min-w-0"><slot /></span>
  </div>
</template>
