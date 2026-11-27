<script setup lang="ts">
import { computed } from 'vue';
import { landingLook } from '../landings';
import type { Landing } from '../core/generated/types.generated';
import StandingBadge from './StandingBadge.vue';

// Where a landing stands, as every page that lists landings draws it: the tone, the icon (the ring while it is checked) and the word.
const props = defineProps<{ landing: Landing }>();
const look = computed(() => landingLook(props.landing));
</script>

<template>
  <StandingBadge :variant="look.tone" :icon="look.icon" :working="look.working" :data-state="landing.state.kind" data-landing-state>{{ look.label }}</StandingBadge>
</template>
