<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { useEventListener, useResizeObserver } from '@vueuse/core';
import { useBoard } from '../board';
import BoardColumn from './BoardColumn.vue';
import Icon from './Icon.vue';
import PageTabs from './PageTabs.vue';
import ProjectFilter from './ProjectFilter.vue';
import SearchBox from './SearchBox.vue';
import Tip from './Tip.vue';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

// Every chain on one calm board, laid out by the server (catalog.board): what waits for you (in the order you should look:
// what blocks an agent first, longest wait first; each card says how soon, why and since when),
// what runs, what waits (to check back, or on people's replies), what is queued, what is done, and last what was put away.
// What waits for you from no chain (files a deleted chain left, the idle watchdog) is in the first column too; what the
// Assistant asks is with that agent, not here. The search and the project (ProjectFilter) narrow every column. A card opens its chain, with the column that holds it beside it (BoardSidebar).
//
// The board shows as many whole columns as the window holds (never one cut at its edge) and snaps to a column as it scrolls;
// at the edge where columns lie out of view, a fade and a button name them and bring the next one in. A window too narrow for
// two shows one column, and a row of the columns' names above it says which one it is and goes to any other. Where the strip
// is scrolled is view state of the page, kept by the browser as any scroll is, not in the URL.
const { columns, query, project, more } = useBoard(() => true);

const strip = ref<HTMLElement>();
const first = ref(0);
const shown = ref(1);
// The width of one column with its gutter, and the index of the first column in view and how many are, read off the strip.
function measure() {
  const el = strip.value;
  const col = el?.firstElementChild as HTMLElement | null | undefined;
  if (!el || !col) return;
  const step = col.offsetWidth + parseFloat(getComputedStyle(el).columnGap);
  first.value = Math.round(el.scrollLeft / step);
  shown.value = Math.max(1, Math.round((el.clientWidth - parseFloat(getComputedStyle(el).paddingLeft) * 2 + step - col.offsetWidth) / step));
}
useResizeObserver(strip, measure);
useEventListener(strip, 'scroll', measure, { passive: true });
const before = computed(() => columns.value.slice(0, first.value));
const after = computed(() => columns.value.slice(first.value + shown.value));
const names = (cols: readonly { label: string }[]) => cols.map((c) => c.label).join(', ');
// The switcher's name for the column in view stays whole in its own row, however the strip got there (a tap, a swipe).
const switcher = ref<HTMLElement>();
watch(first, (i) => switcher.value?.children[i]?.scrollIntoView({ block: 'nearest', inline: 'nearest' }), { flush: 'post' });
// Brings column i to the strip's start; the strip's scroll-behavior decides whether it slides (not under reduced motion).
function goTo(i: number) {
  const el = strip.value;
  const col = el?.children[i] as HTMLElement | undefined;
  if (el && col) el.scrollTo({ left: col.offsetLeft - parseFloat(getComputedStyle(el).paddingLeft) });
}
</script>

<template>
  <section class="@container flex min-h-0 flex-1 flex-col overflow-hidden">
    <PageTabs group="work"><template #end><SearchBox v-model="query" size="sm" class="w-[320px] max-w-full" placeholder="Search chains…" label="Search chains" /><ProjectFilter v-model="project" /></template></PageTabs>
    <nav ref="switcher" class="switcher gap-1 overflow-x-auto px-pane pt-2.5" aria-label="Board columns" data-switcher>
      <Button v-for="(col, i) in columns" :key="col.key" size="sm" :variant="i === first ? 'secondary' : 'ghost'" class="flex-none" :aria-current="i === first ? 'true' : undefined" @click="goTo(i)">
        {{ col.label }}<Badge variant="quiet" size="count">{{ col.count }}</Badge>
      </Button>
    </nav>
    <div class="relative flex min-h-0 flex-1">
      <div ref="strip" class="strip grid min-h-0 flex-1 grid-flow-col gap-gutter overflow-x-auto px-pane pt-2.5 pb-3.5" data-strip>
        <BoardColumn v-for="col in columns" :key="col.key" class="rounded-lg bg-bg2 p-2" :column="col" @more="more(col.key)" />
      </div>
      <div v-if="before.length" class="cue left-0" data-cue="before">
        <Tip :text="`Out of view: ${names(before)}`"><Button variant="outline" size="icon-sm" :aria-label="`Show the column before: ${before.at(-1)?.label}`" @click="goTo(first - 1)"><Icon name="chevron-left" /></Button></Tip>
      </div>
      <div v-if="after.length" class="cue right-0" data-cue="after">
        <Tip :text="`Out of view: ${names(after)}`"><Button variant="outline" size="icon-sm" :aria-label="`Show the next column: ${after[0]?.label}`" @click="goTo(first + 1)"><Icon name="chevron-right" /></Button></Tip>
      </div>
    </div>
  </section>
</template>

<style scoped>
/* As many whole columns as the board's width holds, each at least 300px: the strip's width W holds n when W >= 310n + 22
   (n columns, n - 1 gutters, the strip's padding on both sides). Seven fit, they share the width. */
.strip {
  grid-auto-columns: 100%;
  scroll-snap-type: x mandatory;
  scroll-padding-inline: var(--space-pane);
  scroll-behavior: smooth;
}
.strip > * {
  scroll-snap-align: start;
}
@container (min-width: 642px) {
  .strip {
    grid-auto-columns: calc((100% - var(--space-gutter)) / 2);
  }
}
@container (min-width: 952px) {
  .strip {
    grid-auto-columns: calc((100% - 2 * var(--space-gutter)) / 3);
  }
}
@container (min-width: 1262px) {
  .strip {
    grid-auto-columns: calc((100% - 3 * var(--space-gutter)) / 4);
  }
}
@container (min-width: 1572px) {
  .strip {
    grid-auto-columns: calc((100% - 4 * var(--space-gutter)) / 5);
  }
}
@container (min-width: 1882px) {
  .strip {
    grid-auto-columns: calc((100% - 5 * var(--space-gutter)) / 6);
  }
}
@container (min-width: 2192px) {
  .strip {
    grid-auto-columns: minmax(300px, 1fr);
  }
}
/* The edge where columns lie out of view: the strip's padding there fades into the page, and a button on the edge, halfway
   down where a column is mostly empty, brings the next one in. Only the button takes the pointer. */
.cue {
  position: absolute;
  top: 0;
  bottom: 0;
  width: var(--space-pane);
  pointer-events: none;
}
.cue[data-cue='after'] {
  background: linear-gradient(to right, transparent, var(--bg));
}
.cue[data-cue='before'] {
  background: linear-gradient(to left, transparent, var(--bg));
}
.cue :deep(button) {
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
  box-shadow: var(--shadow-pop);
  pointer-events: auto;
}
.cue[data-cue='after'] :deep(button) {
  right: 2px;
}
.cue[data-cue='before'] :deep(button) {
  left: 2px;
}
/* One column at a time: the row of names says which and goes to any other, and the edge buttons step aside for it. */
.switcher {
  display: none;
  scroll-padding-inline: var(--space-pane);
}
@container (max-width: 641px) {
  .switcher {
    display: flex;
  }
  .cue {
    display: none;
  }
}
@media (prefers-reduced-motion: reduce) {
  .strip {
    scroll-behavior: auto;
  }
}
</style>
