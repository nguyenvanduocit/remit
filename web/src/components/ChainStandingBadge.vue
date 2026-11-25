<script setup lang="ts">
import { chainIcon, statusTone, WAIT_ICON } from '../ui';
import type { ChainStanding } from '../core/generated/types.generated';
import StandingBadge from './StandingBadge.vue';

// Where a chain stands, as the server says (its standing's tone and words), alike on its board card, in an event's
// flow and in the header of a chain. A chain that needs the owner wears the icon of its first wait (WAIT_ICON), so what waits
// never rests on its colour. A place that says more beside the words (since when, when it checks back) puts them in the slot.
// `opening`: the chain is being opened, and the ring says so.
defineProps<{ standing: ChainStanding; opening?: boolean }>();
</script>

<template>
  <StandingBadge :variant="statusTone(standing.tone)" :working="standing.working || opening" :icon="standing.wait ? WAIT_ICON[standing.wait.kind] : chainIcon(standing.tone)"><slot>{{ standing.text }}</slot></StandingBadge>
</template>
