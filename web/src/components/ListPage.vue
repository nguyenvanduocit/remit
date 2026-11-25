<script setup lang="ts">
import { computed, ref, useSlots } from 'vue';
import { useElementSize } from '@vueuse/core';
import { cardDrawer, compactDrawer, COMPACT_BELOW, listSidebar, MAIN_MIN_PERCENT, paneStackBelow, stackedList } from '../panels';
import EmptyState from './EmptyState.vue';
import PageBody from './PageBody.vue';
import PageHead from './PageHead.vue';
import Sidebar from './Sidebar.vue';
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable';

// The shell of a page that lists what exists on the left and shows the one open on the right under its header: the board's
// chains, the Crew pages (CrewPage), the signals, the subscriptions, the events. `name` names the page for the widths the
// owner gave its columns (they keep their width in pixels, the open one takes the rest; the browser remembers them), `empty`
// (or the `empty` slot) what to say when nothing is open. `tabs` sits above the columns, `head` above the list (a filter, a button that starts something), `foot` under the open one and
// `drawer` in a third column beside it; `sidebar` replaces the whole list column (the board's, whose column tabs hold its
// head and its list). Given a `title`, the open one's header is the header every page about one thing has (PageHead): the
// title (`mono` for a name the owner types), its `standing` (badges), then its `actions`; without one the open
// page draws its own header (a chain's, an event's, with their tabs). `busy`: another one is on its way (nav.ts
// onScreen), so what shows still belongs to the old one and steps back under a running bar. A page too narrow for three columns (panels.ts
// COMPACT_BELOW), measured as it is drawn so the Assistant's column counts, drops the list while something is open beside a
// drawer: the open page and the drawer keep the room to be read.
const props = defineProps<{ name: string; open: boolean; empty?: string; title?: string; mono?: boolean; busy?: boolean }>();
const root = ref<HTMLElement | null>(null);
const { width } = useElementSize(root);
const compact = computed(() => width.value > 0 && width.value < COMPACT_BELOW);
const stacked = computed(() => width.value > 0 && width.value < paneStackBelow);
const slots = useSlots();
const folded = computed(() => compact.value && props.open && !!slots.drawer);
const layout = computed(() => stacked.value ? (folded.value ? 'stacked-folded' : 'stacked') : (folded.value ? 'folded' : 'whole'));
</script>

<template>
  <section ref="root" class="flex min-h-0 flex-1 flex-col" :aria-busy="busy">
    <slot name="tabs" />
    <ResizablePanelGroup :key="layout" class="min-h-0 flex-1" :direction="stacked ? 'vertical' : 'horizontal'" :auto-save-id="layout === 'whole' ? name : name + '-' + layout">
      <template v-if="!folded">
        <ResizablePanel :id="`${name}-list`" :order="1" class="flex" v-bind="stacked ? stackedList : listSidebar">
          <slot name="sidebar">
            <Sidebar>
              <template v-if="$slots.head" #head><slot name="head" /></template>
              <slot name="list" />
            </Sidebar>
          </slot>
        </ResizablePanel>
        <ResizableHandle aria-label="Resize the list" />
      </template>
      <ResizablePanel :id="`${name}-page`" :order="2" class="flex" :min-size="MAIN_MIN_PERCENT">
        <div class="relative flex min-w-0 flex-1 flex-col" :class="{ busy }" data-open-page>
          <div v-if="busy" class="loadbar" aria-hidden="true"><i></i></div>
          <template v-if="open">
            <PageHead v-if="title !== undefined" :title="title" :mono="mono">
              <template #standing><slot name="standing" /></template>
              <template #end>
                <slot name="actions" />
              </template>
            </PageHead>
            <slot />
          </template>
          <PageBody v-else><EmptyState><slot name="empty">{{ empty }}</slot></EmptyState></PageBody>
          <slot name="foot" />
        </div>
      </ResizablePanel>
      <!-- The drawer's column stays when nothing shows in it, so the widths do not jump when the first one opens. -->
      <template v-if="$slots.drawer">
        <ResizableHandle aria-label="Resize the drawer" />
        <ResizablePanel :id="`${name}-drawer`" :order="3" class="flex" :class="{ busy }" v-bind="folded ? compactDrawer : cardDrawer"><slot name="drawer" /></ResizablePanel>
      </template>
    </ResizablePanelGroup>
  </section>
</template>

<style scoped>
/* Both wait a moment before they show, so a quick open does not blink; leaving is instant. */
.busy > :slotted(*) {
  opacity: 0.4;
  pointer-events: none;
  transition: opacity 0.15s 0.15s;
}
.loadbar {
  position: absolute;
  inset: 0 0 auto;
  z-index: 3;
  height: 2px;
  overflow: hidden;
  opacity: 0;
  animation: show 0s 0.15s forwards;
}
.loadbar i {
  position: absolute;
  inset: 0 auto 0 0;
  width: 30%;
  background: var(--blue);
  animation: slide 1s ease-in-out infinite;
}
@keyframes show {
  to {
    opacity: 1;
  }
}
@keyframes slide {
  from {
    transform: translateX(-100%);
  }
  to {
    transform: translateX(334%);
  }
}
@media (prefers-reduced-motion: reduce) {
  .loadbar i {
    width: 100%;
    opacity: 0.5;
  }
}
</style>
