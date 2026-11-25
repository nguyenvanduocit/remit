<script lang="ts">
// Remit's icons: inline SVG strokes on one 24-unit grid, one stroke (2 units, round caps and joins), so they look the same on
// every machine, unlike symbol characters, which depend on the fonts a system has (or turn into colour emoji). Two families
// share the grid: interface marks (what a control does) and the domain's own glyphs (what a thing is: a chain, a Chair, a
// gate), drawn for Remit. ICONS is the list; a name outside it draws a plain ring rather than failing.
export const ICONS = {
  check: 'M5 12.5l4.5 4.5L19 7.5',
  plus: 'M12 5v14M5 12h14',
  spark: 'M12 3.5l1.9 5.6 5.6 1.9-5.6 1.9L12 18.5l-1.9-5.6L4.5 11l5.6-1.9z',
  alert: 'M12 4l9 16H3zM12 10v4M12 17.2v.1',
  'chevron-down': 'M6 9.5l6 6 6-6',
  'chevron-up': 'M6 14.5l6-6 6 6',
  'chevron-left': 'M14.5 6l-6 6 6 6',
  'chevron-right': 'M9.5 6l6 6-6 6',
  stop: 'M7 7h10v10H7z',
  interrupt: 'M12 4a8 8 0 1 0 0 16 8 8 0 1 0 0-16M9.5 9.5h5v5h-5z',
  archive: 'M4 5h16v4H4zM5.5 9v10h13V9M10 13h4',
  trash: 'M5 7h14M10 7V5h4v2M7 7l1 12h8l1-12M10.5 10.5v5.5M13.5 10.5v5.5',
  more: 'M6 12h.1M12 12h.1M18 12h.1',
  dot: 'M12 10.2a1.8 1.8 0 1 0 0 3.6 1.8 1.8 0 1 0 0-3.6',
  circle: 'M12 6.5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 1 0 0-11',
  play: 'M8 5.5v13l10.5-6.5z',
  pause: 'M8.5 6v12M15.5 6v12',
  external: 'M14 5h5v5M19 5l-8 8M17 13.5V19H5V7h5.5',
  link: 'M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1.2 1.2M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1.2-1.2',
  eye: 'M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12M12 9.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 1 0 0-5',
  clock: 'M12 4a8 8 0 1 0 0 16 8 8 0 1 0 0-16M12 8v4.5l3 2',
  copy: 'M9 9h10v10H9zM5 15V5h10',
  pencil: 'M5 19l1-4.2L15.6 5.2a2 2 0 0 1 2.8 2.8L8.8 17.6zM13.8 7l3.2 3.2',
  list: 'M9 7h10M9 12h10M9 17h10M5 7h.1M5 12h.1M5 17h.1',
  filter: 'M4 5.5h16l-6.2 7.6v5.4l-3.6 1.5v-6.9z',
  search: 'M10.5 5a5.5 5.5 0 1 0 0 11 5.5 5.5 0 1 0 0-11M14.5 14.5L19 19',
  download: 'M12 5v10M7.5 10.5L12 15l4.5-4.5M5 19h14',
  x: 'M6.5 6.5l11 11M17.5 6.5l-11 11',
  terminal: 'M5 7.5l4.5 4.5L5 16.5M12 17h7',
  file: 'M7 4h7l4 4v12H7zM14 4v4h4',
  paperclip: 'M21.4 11.6l-9.2 9.2a5.5 5.5 0 0 1-7.8-7.8l10-10a3.5 3.5 0 0 1 5 5l-10 10a1.5 1.5 0 0 1-2.1-2.1l9.2-9.2',
  image: 'M4 5h16v14H4zM4 16l4.5-4.5 4 4 2.5-2.5L20 18M15.5 8.5h.1',
  book: 'M5 18.5V5.5A1.5 1.5 0 0 1 6.5 4H19v13H6.5A1.5 1.5 0 0 0 5 18.5 1.5 1.5 0 0 0 6.5 20H19M9 8h6',
  gear: 'M12 9a3 3 0 1 0 0 6 3 3 0 1 0 0-6M12 3.5v2.2M12 18.3v2.2M3.5 12h2.2M18.3 12h2.2M6 6l1.6 1.6M16.4 16.4L18 18M6 18l1.6-1.6M16.4 7.6L18 6',
  'arrow-left': 'M19 12H5M11 6l-6 6 6 6',
  'arrow-right': 'M5 12h14M13 6l6 6-6 6',
  'arrow-up': 'M12 19V5M6 11l6-6 6 6',
  comment: 'M5 6h14v10H11l-4 3v-3H5z',
  help: 'M12 3.5a8.5 8.5 0 1 0 0 17 8.5 8.5 0 1 0 0-17M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6v.6M12 17v.1',
  scale: 'M12 4v16M8 20h8M5 7.5h14M5 7.5l-2.5 6a2.5 2.5 0 0 0 5 0zM19 7.5l-2.5 6a2.5 2.5 0 0 0 5 0z',
  repeat: 'M17 4l3 3-3 3M20 7H8a4 4 0 0 0-4 4M7 20l-3-3 3-3M4 17h12a4 4 0 0 0 4-4',
  key: 'M8 12.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 1 0 0-7M10.5 14l8.5-8.5M16.5 8l2.5 2.5',
  inbox: 'M3 13h5l1.5 2.5h5L16 13h5M6.2 5h11.6L21 13v6H3v-6z',
  chart: 'M5 19h14M7 15.5V11M12 15.5V6M17 15.5v-7',
  person: 'M12 4.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 1 0 0-7M5 19.5a7 7 0 0 1 14 0',
  // The domain. A chain is steps in a row; a Chair the seat that runs it; the Assistant a pair of sparks; an agent a head with
  // an antenna; an employee a badge on a clip; a mentor a person beside a smaller one; the crew three heads.
  chain: 'M5 9.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 1 0 0-5M12 9.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 1 0 0-5M19 9.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 1 0 0-5M7.5 12h2M14.5 12h2',
  chair: 'M8 4h8v8H8zM5.5 12h13v3h-13zM7.5 15v5M16.5 15v5',
  assistant: 'M10 4l1.6 4.4L16 10l-4.4 1.6L10 16l-1.6-4.4L4 10l4.4-1.6zM17.5 14l.9 2.1 2.1.9-2.1.9-.9 2.1-.9-2.1-2.1-.9 2.1-.9z',
  agent: 'M6 8.5h12v10H6zM12 8.5V5M12 4v.1M9.5 13v1.5M14.5 13v1.5M3.5 12.5v3M20.5 12.5v3',
  employee: 'M5.5 6h13v14h-13zM10 6V3.5h4V6M12 9.5a2 2 0 1 0 0 4 2 2 0 1 0 0-4M8.5 17.5a3.5 3.5 0 0 1 7 0',
  mentor: 'M9 4.5a3 3 0 1 0 0 6 3 3 0 1 0 0-6M3.5 19.5a5.5 5.5 0 0 1 11 0M17 9.5a2 2 0 1 0 0 4 2 2 0 1 0 0-4M15.5 19.5h5a2.5 2.5 0 0 0-5 0',
  crew: 'M12 4.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 1 0 0-5M5.5 8a2 2 0 1 0 0 4 2 2 0 1 0 0-4M18.5 8a2 2 0 1 0 0 4 2 2 0 1 0 0-4M7.5 19a4.5 4.5 0 0 1 9 0M2.5 18a3 3 0 0 1 4.5-2.6M21.5 18a3 3 0 0 0-4.5-2.6',
  // What a chain passes and is judged by: a gate across its way, the judge's gavel, a release taking off, a merge of two lines.
  gate: 'M5 4v16M19 4v16M5 8h14M5 16h14M5 16L19 8',
  judge: 'M13.5 3.5l6 6-3 3-6-6zM12 8l-7.5 7.5 3 3L15 11M13 20.5h7',
  release: 'M12 3c3 2.2 4.5 5.4 4.5 9.5L15 15.5H9l-1.5-3C7.5 8.4 9 5.2 12 3zM12 9v.1M9 15.5l-1.5 4.5 3-1.5M15 15.5l1.5 4.5-3-1.5',
  merge: 'M7 3.5a2 2 0 1 0 0 4 2 2 0 1 0 0-4M7 7.5v13M17 14.5a2 2 0 1 0 0 4 2 2 0 1 0 0-4M7 9.5c0 3.5 3.5 6.5 8 6.5',
  // What starts and steers work: a signal sent out, a subscription's bell, an event's bolt, a schedule's page, an objective's
  // target, a project's folder, a wait's hourglass.
  signal: 'M12 11a1.5 1.5 0 1 0 0 3 1.5 1.5 0 1 0 0-3M12 14v6M8.5 9a5 5 0 0 0 0 7M15.5 9a5 5 0 0 1 0 7M5.5 6a9 9 0 0 0 0 13M18.5 6a9 9 0 0 1 0 13',
  subscription: 'M6 16.5V11a6 6 0 0 1 12 0v5.5l1.5 2h-15zM10 21h4',
  event: 'M13.5 3L5.5 13.5h6L10.5 21l8-10.5h-6z',
  schedule: 'M4.5 6h15v14h-15zM4.5 10.5h15M8.5 3.5v5M15.5 3.5v5M8.5 14.5h.1M12 14.5h.1M15.5 14.5h.1',
  objective: 'M12 4a8 8 0 1 0 0 16 8 8 0 1 0 0-16M12 8a4 4 0 1 0 0 8 4 4 0 1 0 0-8M12 12v.1',
  project: 'M3.5 6.5v12h17V8.5H11.5l-2-2zM3.5 11h17',
  waits: 'M7 3.5h10M7 20.5h10M8 3.5c0 4.5 8 5 8 8.5s-8 4-8 8.5M16 3.5c0 4.5-8 5-8 8.5s8 4 8 8.5',
  // What agents are made of and reach with: a playbook's clipboard, a skill's medal, memory's stack of records, a source's
  // plug, an extension's box.
  playbook: 'M9 3.5h6v3H9zM9 5H6v15.5h12V5h-3M9 11h6M9 15h4',
  skill: 'M12 10a5 5 0 1 0 0 10 5 5 0 1 0 0-10M9 10.5L7 3.5h3.5L12 7l1.5-3.5H17l-2 7',
  memory: 'M5 6.5C5 5.1 8.1 4 12 4s7 1.1 7 2.5S15.9 9 12 9 5 7.9 5 6.5M5 6.5v11c0 1.4 3.1 2.5 7 2.5s7-1.1 7-2.5v-11M5 12c0 1.4 3.1 2.5 7 2.5s7-1.1 7-2.5',
  source: 'M9 3.5v4M15 3.5v4M6.5 7.5h11v3a5.5 5.5 0 0 1-11 0zM12 16v4.5',
  extension: 'M12 3.5l8 4.5v8l-8 4.5L4 16V8zM4 8l8 4.5L20 8M12 12.5v8',
} as const;
export type IconName = keyof typeof ICONS;
</script>

<script setup lang="ts">
import { computed } from 'vue';

// `name` is a string, as the looks that pick one (ui.ts, the column and kind tables) hold their names as strings; a map that
// wants its names checked types them `IconName`. `label` is what a screen reader says for an icon that stands alone (no words
// beside it); without one the icon is decoration and the words beside it say it.
const props = withDefaults(defineProps<{ name: string; size?: number; label?: string }>(), { size: 14 });
const known = (name: string): name is IconName => Object.hasOwn(ICONS, name);
const path = computed(() => ICONS[known(props.name) ? props.name : 'circle']);
</script>

<template>
  <svg class="icon" :width="props.size" :height="props.size" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" :role="label ? 'img' : undefined" :aria-label="label" :aria-hidden="label ? undefined : 'true'"><path :d="path" /></svg>
</template>

<style scoped>
/* On the text baseline, never squeezed by a flex row. */
.icon {
  display: inline-block;
  flex: none;
  vertical-align: -2px;
}
</style>
