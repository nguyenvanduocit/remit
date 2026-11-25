<script setup lang="ts">
import { computed } from 'vue';
import { LANDING_LOOK } from '../landings';
import type { LandingState } from '../core/generated/types.generated';
import StandingBadge from './StandingBadge.vue';

// Where a landing stands, as every page that lists landings draws it: the tone, the icon (the ring while it is checked) and the word.
const props = defineProps<{ state: LandingState }>();
const look = computed(() => LANDING_LOOK[props.state.kind]);
</script>

<template>
  <StandingBadge :variant="look.tone" :icon="look.icon" :working="look.working" :data-state="state.kind" data-landing-state>{{ look.label }}</StandingBadge>
</template>
