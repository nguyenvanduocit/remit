<script setup lang="ts">
import type { RouteLocationRaw } from 'vue-router';

// One row of a side list, a link to what it names (a chain, a playbook): its title (what it holds sits in one line, centred),
// its `description` cut at two lines (the page it opens says it in full), a line of what else to know, and what the list adds
// below (an event's outcome and the chains it reached). `on` marks the one open. The link covers the whole row; what is
// below sits above it, so it can hold controls of its own (a link may not hold one), while a click on its text falls
// through to the row's link. What the link holds sits above it too, so a hint on its text opens on hover (Tip) while a
// click still lands on the link. The list it sits in spaces the rows.
defineProps<{ to: RouteLocationRaw; on: boolean; description?: string }>();
</script>

<template>
  <div class="row" :class="{ sel: on }">
    <RouterLink class="link" :to="to" :aria-current="on ? 'page' : undefined" data-row>
      <div class="title"><slot name="title" /></div>
      <p v-if="description" class="mt-1 line-clamp-2 text-[11.5px] text-mut [overflow-wrap:anywhere]" data-description>{{ description }}</p>
      <div v-if="$slots.default" class="meta"><slot /></div>
    </RouterLink>
    <div v-if="$slots.below" class="below"><slot name="below" /></div>
  </div>
</template>

<style scoped>
.row {
  position: relative;
  isolation: isolate;
  padding: 10px 12px;
  border: 1px solid transparent;
  border-radius: 8px;
}
.row:hover {
  background: var(--panel);
}
/* the chosen row is pressed into the list, as the open card is (ItemCard) */
.row.sel {
  background: var(--well);
  box-shadow: var(--shadow-pressed);
}
.link {
  display: block;
  color: inherit;
  text-decoration: none;
}
.link::after {
  content: '';
  position: absolute;
  inset: 0;
}
.below,
.link > * {
  position: relative;
  z-index: 1;
}
.below {
  pointer-events: none;
}
.below :deep(:is(a, button, input, select, textarea, [tabindex])) {
  pointer-events: auto;
}
.title {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  font-size: 12.5px;
  font-weight: 600;
  line-height: 1.35;
}
.meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 2px 3px;
  margin-top: 4px;
  font-size: 11.5px;
  color: var(--mut);
}
</style>
