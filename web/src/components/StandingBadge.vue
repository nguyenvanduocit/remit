<script setup lang="ts">
import { Badge, type BadgeVariants } from '@/components/ui/badge';
import Icon from './Icon.vue';
import Spinner from './Spinner.vue';

// Where something stands, the one badge every page draws it with: its colour, the ring while it is at work, else its icon,
// and its words. What colour and icon a chain, an agent or an event takes is decided by the badge for it
// (ChainStandingBadge, AgentStandingBadge), so a page never picks one itself.
defineProps<{ variant: BadgeVariants['variant']; working?: boolean; icon?: string }>();
</script>

<template>
  <Badge :variant="variant" class="min-w-0"><Spinner v-if="working" /><Icon v-else-if="icon" :name="icon" :size="12" /><span class="truncate"><slot /></span></Badge>
</template>
