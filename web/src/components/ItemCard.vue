<script setup lang="ts">
import type { RouteLocationRaw } from 'vue-router';
import Tip from './Tip.vue';

// The card every board's items wear (a chain, a subscription, an event, a piece of feedback): the surface with its hairline,
// lit under the pointer, the one open beside its column pressed into the page (--well, --shadow-pressed) and never ringed. The link to the
// item's page covers the whole card. Given a `title`, the card lays itself out: the title across the card's whole width (cut
// at two lines, the hint says it whole; `mono` for a name the owner types), and under it one line of where it stands: the `chip`
// of what kind it is, then the `badge`, never cut, then the `meta` (what else to know, the time at its end), wrapping when the
// card is narrow. Without a title, the default slot lays the link out itself. What may hold links of its own (an outcome in
// Markdown, a merge's next steps) goes in `below`, above the link, so those can be clicked while a click beside them opens the item. The column it
// sits in spaces the cards.
defineProps<{ to: RouteLocationRaw; on: boolean; title?: string; mono?: boolean }>();
</script>

<template>
  <div class="card" :class="{ on }">
    <RouterLink class="link flex items-start gap-2.5" :to="to" :aria-current="on ? 'page' : undefined" data-row>
      <template v-if="title !== undefined">
        <span class="flex min-w-0 flex-1 flex-col gap-1">
          <Tip :text="title" when-clipped><span class="line-clamp-2 leading-[1.35] font-semibold [overflow-wrap:anywhere]" :class="mono ? 'font-mono text-[12px]' : 'text-[12.5px]'" data-title>{{ title }}</span></Tip>
          <span class="meta flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1 text-[11.5px] leading-[1.4]" data-meta><slot name="chip" /><slot name="badge" /><slot name="meta" /></span>
        </span>
      </template>
      <slot v-else />
    </RouterLink>
    <div v-if="$slots.below" class="below"><slot name="below" /></div>
  </div>
</template>

<style scoped>
.card {
  position: relative;
  isolation: isolate;
  padding: 9px 10px;
  border-radius: 8px;
  background: var(--panel);
  box-shadow: var(--shadow-card);
}
.card:hover {
  box-shadow: var(--shadow-card-hover);
}
.card.on {
  background: var(--well);
  box-shadow: var(--shadow-pressed);
}
.link {
  min-width: 0;
  color: inherit;
  text-decoration: none;
}
.link::after {
  content: '';
  position: absolute;
  inset: 0;
}
.below,
.link > :slotted(*),
.link > * {
  position: relative;
  z-index: 1;
}
.below {
  pointer-events: none;
}
.below :deep(a),
.below :deep(button) {
  pointer-events: auto;
}
/* The line under the title is muted, 4.5:1 or more on the card and on the pressed one alike. */
.meta {
  color: var(--mut);
}
</style>
