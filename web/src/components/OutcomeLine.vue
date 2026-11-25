<script setup lang="ts">
import { decisionIcon } from '../decisionRows';
import { useStream } from '../streamContext';
import { hm } from '../ui';
import type { Decision } from '../core/generated/types.generated';
import ActorTag from './ActorTag.vue';
import Icon from './Icon.vue';

// One outcome of the chain where it happened in its stream (ADR-0081): what was done in the server's words, who did it, and
// why when it has a why ("Merged !12: stage 5% · Chair", "Archive chain. · You · duplicate of Backfill zone groups").
defineProps<{ decision: Decision }>();
const stream = useStream();
</script>

<template>
  <div class="outcome flex min-w-0 items-start gap-2 px-3 py-1.5 text-[12px] text-text2" data-outcome :data-kind="decision.kind">
    <Icon class="mt-0.5 flex-none text-mut" :name="decisionIcon[decision.kind] ?? 'dot'" :size="12" />
    <p class="min-w-0 flex-1 [overflow-wrap:anywhere]">
      <span class="font-medium text-text">{{ decision.text }}</span>{{ ' ' }}<ActorTag v-if="decision.by" class="mx-0.5 align-[1px]" :by="decision.by" :name-of="stream.nameOf" />{{ ' ' }}<span v-if="decision.why" class="text-mut">· {{ decision.why }}</span>
    </p>
    <time class="flex-none text-[11px] text-mut tabular-nums" :datetime="decision.createdAt">{{ hm(decision.createdAt) }}</time>
  </div>
</template>

<style scoped>
/* A mark between the runs of the feed, on the quieter surface: not a run of an agent's work. */
.outcome {
  border-radius: 8px;
  background: var(--panel2);
}
</style>
