<script setup lang="ts">
import Fold from './Fold.vue';
import { Badge } from '@/components/ui/badge';

// A part of an agent's context folded under its name (and how many it holds, or what it costs in the note).
defineProps<{ title: string; count?: number; open?: boolean }>();
</script>

<template>
  <Fold :open="open">
    <template #summary>
      <span class="flex min-w-0 items-center gap-1.5 py-1.5 text-[12.5px] text-text2">
        <span class="font-semibold text-text">{{ title }}</span>
        <Badge v-if="count !== undefined" variant="quiet" size="count">{{ count }}</Badge>
        <span v-if="$slots.note" class="min-w-0 truncate text-[11.5px] text-mut"><slot name="note" /></span>
      </span>
    </template>
    <div class="pb-2"><slot /></div>
  </Fold>
</template>
