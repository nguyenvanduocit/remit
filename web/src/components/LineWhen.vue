<script setup lang="ts">
import { clockAt, timed } from '../ui';
import type { SubscriptionLine } from '../core/generated/types.generated';
import Until from './Until.vue';

// What a subscription waits for, as the owner reads it: a time (how far off, kept true as time passes, and at what time; a
// paused one keeps its planned time and says so; one that has begun is happening now), a watch's deadline and the reading that
// wakes it sooner, or an event subscription's events: every one, or those its condition matches.
defineProps<{ line: SubscriptionLine }>();
</script>

<template>
  <span v-if="line.state === 'underway'">Now</span>
  <span v-else-if="line.state === 'over'">No next run</span>
  <span v-else-if="line.kind === 'event'">{{ line.condition ? 'Each event its condition matches' : 'Every event' }}</span>
  <span v-else-if="!timed(line.at)">At a reading that meets its condition</span>
  <span v-else-if="line.state === 'paused'">Paused · planned {{ clockAt(line.at) }}</span>
  <span v-else><template v-if="line.kind === 'watch'">Deadline </template><Until :at="line.at" /> · {{ clockAt(line.at) }}</span>
</template>
