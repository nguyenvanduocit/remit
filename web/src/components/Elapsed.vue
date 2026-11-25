<script setup lang="ts">
import { computed } from 'vue';
import { useClock } from '../clock';
import { dur } from '../ui';

// How long something took, from `from` to `to`, or to now while it goes on: a leaf of its own, so the clock re-renders
// this text and nothing around it.
const props = defineProps<{ from: string; to?: string }>();
const now = useClock();
const text = computed(() => dur((props.to ? new Date(props.to).getTime() : now.value.getTime()) - new Date(props.from).getTime()));
</script>

<template>
  <span>{{ text }}</span>
</template>
