<script setup lang="ts">
import Icon from './Icon.vue';
import StandingMark from './StandingMark.vue';
import type { Tone } from '../ui';

// One step an agent took with a tool, as a card: its icon, the name it is shown by, what it did, where it stands and how
// long it took on one line the owner opens for the details (the default slot). A tool call and a background task the
// agent started (TaskRow) are drawn by this one card, so they read the same; `actions` sits beside the line, outside
// its button (a task's Stop).
defineProps<{ icon: string; name: string; summary: string; look: { tone: Tone; icon?: string; label?: string; working?: boolean }; time: string; failed?: boolean }>();
const open = defineModel<boolean>('open', { default: false });
</script>

<template>
  <div class="call" :class="{ failed }">
    <div class="flex min-w-0 items-center">
      <button type="button" class="call-h" :aria-expanded="open" @click="open = !open">
        <Icon class="w-4 flex-none text-mut" :name="icon" :size="13" />
        <span class="mono flex-none text-text">{{ name }}</span>
        <span class="min-w-0 flex-1 truncate text-mut">{{ summary }}</span>
        <StandingMark v-bind="look" />
        <span class="ms">{{ time }}</span>
      </button>
      <slot name="actions" />
    </div>
    <div v-if="open" class="border-t border-line px-2.5 py-[7px]"><slot /></div>
  </div>
</template>

<style scoped>
.call {
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--panel);
}
.call.failed {
  border-color: var(--red-line);
}
.call-h {
  display: flex;
  flex: 1;
  align-items: center;
  gap: 8px;
  min-width: 0;
  padding: 6px 9px;
  font-size: 12px;
  text-align: left;
  cursor: pointer;
}
.ms {
  flex: none;
  min-width: 30px;
  font-size: 11px;
  color: var(--faint);
  text-align: right;
  font-variant-numeric: tabular-nums;
}
</style>
