<script setup lang="ts">
import { computed } from 'vue';
import { useClock } from '../clock';
import { ago } from '../ui';

// How long ago something happened, kept true as time passes: a leaf of its own, so the clock re-renders this text and
// nothing around it.
const props = defineProps<{ at: string }>();
const now = useClock();
const text = computed(() => ago(props.at, now.value.getTime()));
</script>

<template>
  <span>{{ text }}</span>
</template>
