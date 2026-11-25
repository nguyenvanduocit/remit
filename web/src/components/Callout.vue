<script setup lang="ts">
import { computed } from 'vue';
import IconChip from './IconChip.vue';

// A boxed notice in a page or a feed: what waits on the owner (`ask`), why the Chair did something (`note`), what went wrong
// (`error`). The box is a card like any other, neutral; its tone is a chip, an icon on a tint in the tone's hue (orange,
// sand, red, ADR-0090), beside the heading or, with no heading, beside the body, and the heading's words say it too, so the
// tone is never told by colour alone. `icon` names the chip's icon, else the tone's own. The title slot is the heading, the
// default slot the body; a <pre> in the body is a block of output.
const props = withDefaults(defineProps<{ tone?: 'ask' | 'note' | 'error'; icon?: string }>(), { tone: 'ask' });
const LOOK = {
  ask: { icon: 'waits', tint: 'bg-orange-tint text-orange' },
  note: { icon: 'comment', tint: 'bg-sand-tint text-sand' },
  error: { icon: 'alert', tint: 'bg-red-tint text-red' },
} as const;
const look = computed(() => LOOK[props.tone]);
</script>

<template>
  <div class="callout rounded-lg bg-panel p-card" :data-tone="tone">
    <template v-if="$slots.title">
      <h4 class="mb-1.5 flex items-start gap-icon"><IconChip :icon="icon ?? look.icon" :class="look.tint" /><span class="min-w-0 flex-1 pt-px [overflow-wrap:anywhere]"><slot name="title" /></span></h4>
      <slot />
    </template>
    <div v-else class="flex items-start gap-icon"><IconChip :icon="icon ?? look.icon" :class="look.tint" /><div class="min-w-0 flex-1 pt-px"><slot /></div></div>
  </div>
</template>

<style scoped>
.callout {
  box-shadow: var(--shadow-card);
}
h4 {
  font-size: 13px;
  font-weight: 600;
  line-height: 1.4;
  color: var(--text);
}
:slotted(pre) {
  margin: 6px 0 8px;
  padding: 7px 8px;
  border: 1px solid var(--line);
  border-radius: 7px;
  background: var(--bg);
  font-family: var(--mono);
  font-size: 11px;
  color: var(--text2);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
</style>
