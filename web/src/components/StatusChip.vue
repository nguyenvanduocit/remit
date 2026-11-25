<script setup lang="ts">
import { STATUS_LABEL, statusTone } from '../ui';
import type { CardStatus, ChainStatus, WorkflowStatus } from '../core/generated/types.generated';
import StandingBadge from './StandingBadge.vue';
// Where something stands, and what else to know about it at a glance (`detail`: a queued chain's place in the queue).
defineProps<{ status: ChainStatus | CardStatus | WorkflowStatus; detail?: string }>();
</script>

<template>
  <StandingBadge :variant="statusTone(status)" :working="status === 'running'">{{ STATUS_LABEL[status] ?? status }}<template v-if="detail"> · {{ detail }}</template></StandingBadge>
</template>
