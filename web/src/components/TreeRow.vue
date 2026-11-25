<script setup lang="ts">
import type { Component } from 'vue';
import { INDENT, ROW, ROW_PAD } from '../tree';
import Tip from './Tip.vue';

// One line of an outline (Outline), `level` steps in, the frame every row of one is drawn in: an agent in a chain's tree (AgentRow). Its mark (an agent's badge, a RowMark) stands where
// the outline's lines meet it, then its name, what the row adds beside the name (`meta`), its line (what it is doing, why
// it stands where it stands: `line`, muted; a row that draws its line itself puts it in the `line` slot and
// keeps `line` as its words for the hint), and at the end what the row ends with (its badge, turns, time).
// `as` is the element the row is (a link for a row that goes somewhere); `picked` is the open one, its lighter surface;
// `muted` is a row nothing opens.
withDefaults(defineProps<{ level: number; as?: string | Component; picked?: boolean; muted?: boolean; line?: string }>(), { as: 'li', line: '' });
</script>

<template>
  <component :is="as" class="row flex items-center gap-2 pr-1" :class="{ sel: picked, muted }" :style="{ height: ROW + 'px', paddingLeft: ROW_PAD + level * INDENT + 'px' }" :aria-level="level + 1">
    <slot name="mark" />
    <span class="name"><slot name="name" /></span>
    <slot name="meta" />
    <Tip :text="line" when-clipped><span class="line text-mut"><slot name="line">{{ line }}</slot></span></Tip>
    <slot />
  </component>
</template>

<style scoped>
/* A line of fixed height, so an outline's lines follow from the rows alone (tree.ts). The picked row is its lighter
   surface. */
.row {
  container-type: inline-size;
  border-radius: 6px;
  color: var(--text2);
  font-size: 13px;
  text-decoration: none;
  cursor: pointer;
}
.row:hover {
  background: var(--panel);
}
/* the chosen row is pressed into the list, as the open card is (ItemCard) */
.row.sel {
  background: var(--well);
  box-shadow: var(--shadow-pressed);
}
/* A row nothing opens is quieter in text colour, not in opacity: dimming the row would drop its muted line under 4.5:1. */
.row.muted {
  cursor: default;
}
.row.muted .name {
  color: var(--text2);
}
.row.muted:hover {
  background: none;
}
.name {
  display: block;
  flex: 0 1 auto;
  min-width: 48px;
  max-width: 280px;
  font-weight: 500;
  color: var(--text);
}
.line {
  display: block;
  flex: 1 1 0;
  min-width: 0;
  overflow: hidden;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
