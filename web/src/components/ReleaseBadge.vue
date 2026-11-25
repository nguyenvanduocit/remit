<script setup lang="ts">
import { computed } from 'vue';
import { RELEASE_LOOK } from '../releases';
import type { ReleaseState } from '../core/generated/types.generated';
import StandingBadge from './StandingBadge.vue';

// Where a release stands, as every page that shows releases draws it: the tone, the icon (the ring while its steps run) and the word.
const props = defineProps<{ state: ReleaseState }>();
const look = computed(() => RELEASE_LOOK[props.state.kind]);
</script>

<template>
  <StandingBadge :variant="look.tone" :icon="look.icon" :working="look.working" :data-state="state.kind" data-release-state>{{ look.label }}</StandingBadge>
</template>
