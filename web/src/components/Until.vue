<script setup lang="ts">
import { computed } from 'vue';
import { useClock } from '../clock';
import { until } from '../ui';

// How far off a time is, kept true as time passes: a leaf of its own, so the clock re-renders this text and nothing around it.
const props = defineProps<{ at: string }>();
const now = useClock();
const text = computed(() => until(new Date(props.at).getTime() - now.value.getTime()));
</script>

<template>
  <span>{{ text }}</span>
</template>
