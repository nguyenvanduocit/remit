<script setup lang="ts">
import { computed } from 'vue';
import { SUBSCRIPTION_LOOK, timed } from '../ui';
import { CONTROL_LOOK, HOLD_LOOK } from '../subscriptionWords';
import type { SubscriptionLine } from '../core/generated/types.generated';
import StandingBadge from './StandingBadge.vue';
import Until from './Until.vue';

// Where a subscription stands, the one badge every row and page draws it with. One that does not act says so (paused, ended),
// whatever its state; one held in place on a failed firing waits for the owner, in his orange; else its state the owner has to
// notice (held, due, under way with the ring turning, over) in its colour, a paused one in none, and one that waits with how far
// off its time is (kept true as time passes), a watch with no deadline with the reading it waits for, an event signal with its
// every event.
const props = defineProps<{ line: SubscriptionLine }>();
const look = computed(() => (props.line.control ? CONTROL_LOOK[props.line.control] : props.line.hold ? HOLD_LOOK : SUBSCRIPTION_LOOK[props.line.state]));
</script>

<template>
  <StandingBadge v-if="look" :variant="look.tone" :working="!line.control && !line.hold && line.state === 'underway'" :icon="look.icon" class="flex-none">{{ look.label }}</StandingBadge>
  <StandingBadge v-else-if="line.state === 'paused'" variant="quiet" icon="pause" class="flex-none">Paused</StandingBadge>
  <StandingBadge v-else variant="outline" class="flex-none tabular-nums" data-until>
    <template v-if="line.kind === 'event'">{{ line.condition ? 'On a match' : 'Every event' }}</template>
    <template v-else-if="!timed(line.at)">At a reading</template>
    <Until v-else :at="line.at" />
  </StandingBadge>
</template>
