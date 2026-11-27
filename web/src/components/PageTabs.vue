<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, useTemplateRef } from 'vue';
import { useRoute } from 'vue-router';
import { catalog } from '../catalog';
import { routes, type Area } from '../router';
import { Badge } from '@/components/ui/badge';

// The pages one top-bar entry stands for, as a strip of links: the routes of that area that carry a `tab` (router.ts), in
// their order; a page under one of them (`under`) marks its link. They are navigation, not tabs of one page: each a link to its page (middle-click and Ctrl-click work, the open one
// says `aria-current`), and the arrow keys do nothing, Tab walks the strip as it walks the top bar. `end` is what a page keeps at the
// strip's end (the board's search), so the strip takes no row of its own from it. A link counts what waits for the owner on its
// page, as the server counts it, in his orange and only while there is any: Subscriptions the subscriptions held in place on a
// failed firing (SignalBoard.held).
const props = defineProps<{ group: Area }>();
const route = useRoute();
const pages = routes.filter((r) => r.meta?.area === props.group && r.meta.tab).map((r) => ({ name: String(r.name), label: String(r.meta?.tab) }));
const waiting = computed<Partial<Record<string, number>>>(() => ({ subscriptions: catalog.signals?.held }));
const current = computed(() => route.meta.under ?? String(route.name));

// Each page mounts the strip itself, so a link opening another page swaps one strip for the next. A focus that is in the old
// strip as it leaves (Enter on a link) lands on the open page's link of the new one, so the keyboard keeps its place.
const root = useTemplateRef<HTMLElement>('root');
onBeforeUnmount(() => {
  if (!root.value?.contains(document.activeElement)) return;
  focusHandedOn = true;
  // The strip that takes its place mounts in this same flush; one that does not leaves nothing to hand on.
  void nextTick(() => (focusHandedOn = false));
});
onMounted(() => {
  const link = root.value?.querySelector<HTMLElement>(`a[data-page="${current.value}"]`);
  link?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  if (!focusHandedOn) return;
  focusHandedOn = false;
  link?.focus({ preventScroll: true });
});
</script>

<script lang="ts">
// Whether the strip that just left held the focus, for the strip that takes its place.
let focusHandedOn = false;
</script>

<template>
  <div ref="root" class="flex flex-wrap items-center gap-x-4 border-b border-line bg-bg2 px-3.5">
    <nav class="flex h-9 min-w-0 max-w-full items-stretch gap-4 overflow-x-auto" aria-label="Pages">
      <RouterLink
        v-for="p in pages"
        :key="p.name"
        :to="{ name: p.name }"
        class="link relative inline-flex flex-none items-center gap-1.5 px-0.5 text-[12.5px] font-medium whitespace-nowrap text-mut hover:text-text aria-[current=page]:text-text"
        :aria-current="p.name === current ? 'page' : undefined"
        :aria-label="waiting[p.name] ? `${p.label}, ${waiting[p.name]} waiting for you` : undefined"
        :data-page="p.name"
        >{{ p.label }}<Badge v-if="waiting[p.name]" size="count" variant="needs" data-tab-count>{{ waiting[p.name] }}</Badge></RouterLink
      >
    </nav>
    <div v-if="$slots.end" class="flex min-h-9 min-w-0 flex-[1_1_16rem] items-center justify-end gap-2"><slot name="end" /></div>
  </div>
</template>

<style scoped>
/* An inset underline and focus outline stay inside the scrolling strip. */
.link::after {
  content: '';
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  height: 2px;
  background: var(--primary);
  opacity: 0;
}
.link:focus-visible {
  outline-offset: -2px;
}
.link[aria-current='page']::after {
  opacity: 1;
}
</style>
