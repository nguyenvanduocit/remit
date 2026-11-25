<script setup lang="ts">
import { computed, provide, ref, watch, watchEffect } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useEventListener, useMediaQuery } from '@vueuse/core';
import { CHORD_MS, keyAct, typingAt } from '../appKeys';
import { backToRow, walkRows } from '../listKeys';
import { toast } from '../act';
import { useWaitNotifications } from '../notify';
import { useOpenWait, waiting } from '../waits';
import { useAssistant, useNewChain } from '../nav';
import { catalog } from '../catalog';
import { opened } from '../openChain';
import { pageNamed } from '../pageHeading';
import { pageTitle } from '../pageTitle';
import { assistant } from '../assistant';
import { said } from '../banner';
import { ask } from '../replica';
import { session } from '../session';
import { assistantColumn, MAIN_MIN_PERCENT, paneStackBelow, stackedAssistant } from '../panels';
import AssistantPanel from './AssistantPanel.vue';
import CommandPalette from './CommandPalette.vue';
import KeysHelp from './KeysHelp.vue';
import TopBar from './TopBar.vue';
import HealthBar from './HealthBar.vue';
import ConnectionBanner from './ConnectionBanner.vue';
import NewChain from './NewChain.vue';
import EmptyState from './EmptyState.vue';
import PageBody from './PageBody.vue';
import Icon from './Icon.vue';
import Spinner from './Spinner.vue';
import { Button } from '@/components/ui/button';
import { Toaster } from '@/components/ui/sonner';
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable';
import { TooltipProvider } from '@/components/ui/tooltip';

// Until the first snapshot has been had and while no session is up, only what the connection is doing, centered: there is
// nothing yet to show. Once a replica is held, the signed-in app, whatever the link does (ADR-0094): when the link goes, the
// page stays as it was, read-only, and the connection banner says since when and how it reconnects; what would change
// something says why it cannot while the link is down, and a message still goes, once the link is back. The app: the top
// bar, the connection banner while what is shown may not be current, the health bar when agents
// cannot run, the routed view (the page's one main landmark, which a skip link at the very top moves focus to) once the first
// snapshot is in and a loading line until then, so no page draws the empty it would show for what it does not know yet; the Assistant beside the view or below it on a narrow screen (?assistant=1: a chat
// the owner keeps working beside, so it takes room from the view rather than covering it) at the width he dragged it to (panels.ts), the
// new-chain layer over them (?new=1) and the feedback lines at the bottom center. The badge counts what the server's
// board says needs the owner, and a screen reader is told when that count rises (a live region), as the tab's title and
// the Work badge show it.
const stacked = useMediaQuery(`(width < ${paneStackBelow}px)`);
const toReview = computed(() => catalog.board?.needsYou ?? 0);
// A chain's page is named by the chain (its title, "Chain" until it is loaded); every other page by its tab, or its title. The name is the tab's
// title and the page's one h1, which no page draws itself (a header a page draws is a heading beneath it: pageHeading.ts).
provide(pageNamed, true);
const route = useRoute();
const page = computed(() => {
  if (route.name !== 'chain') return route.meta.tab ?? route.meta.title;
  return opened.view?.chain.chain.id === route.params.id ? opened.view.chain.chain.title : 'Chain';
});
watchEffect(() => (document.title = pageTitle(toReview.value, page.value)));
const waits = ref('');
watch(toReview, (n, was) => (waits.value = n > was ? `${n} now need you` : ''));
const main = ref<HTMLElement>();
const { open: creating, close: closeNew } = useNewChain();
const { open: assisting, show: showAssistant } = useAssistant();
const showsAssistant = computed(() => assisting.value && !!assistant.view);
const offline = computed(() => session.link.phase === 'offline');
// Nothing to show: no session, and no snapshot had yet.
const blank = computed(() => offline.value && !catalog.loaded);
// A page of an older build than the server's is turned away until it loads the newer one: nothing dials again, so it waits on
// the owner's Reload, not a spinner (ADR-0081).
const reload = () => location.reload();

// The app's keys (appKeys.ts) and what they open: the palette (⌘K, or the top bar's Find), the list of keys (?), the next thing
// that needs the owner (n, the first of the server's list), the board and the Assistant (g b, g a), and the rows of the list
// the owner is in (j, k, Esc: listKeys.ts). And a browser notification
// for what newly needs him, once he has turned them on (notify.ts).
const router = useRouter();
const openWait = useOpenWait();
const finding = ref(false);
const listingKeys = ref(false);
let afterG = false;
let chord: ReturnType<typeof setTimeout> | undefined;
useEventListener(window, 'keydown', (e: KeyboardEvent) => {
  if (blank.value || e.defaultPrevented || e.isComposing) return;
  const { act, afterG: next } = keyAct(e, typingAt(e.target), afterG);
  afterG = next;
  clearTimeout(chord);
  if (afterG) chord = setTimeout(() => (afterG = false), CHORD_MS);
  if (!act) return;
  // Esc is the row's only when there is a row to go back to; else it stays whatever else on the page takes it for.
  if (act === 'back') return void (backToRow(document.activeElement) && e.preventDefault());
  if ((act === 'down' || act === 'up') && !walkRows(act === 'down' ? 1 : -1, document.activeElement)) return;
  e.preventDefault();
  if (act === 'down' || act === 'up') return;
  if (act === 'palette') finding.value = !finding.value;
  else if (act === 'keys') listingKeys.value = true;
  else if (act === 'board') void router.push({ name: 'board' });
  else if (act === 'assistant') showAssistant();
  else if (waiting.value[0]) openWait(waiting.value[0]);
  else toast('Nothing needs you');
});
useWaitNotifications();
</script>

<template>
  <TooltipProvider :delay-duration="300">
    <div v-if="blank" class="flex h-full flex-col items-center justify-center gap-3 px-6 text-[13px] text-mut" data-phase="offline">
      <!-- The countdown is seen, not heard: a screen reader is told once what the page waits for. -->
      <p role="status" class="sr-only">{{ session.banner?.announce }}</p>
      <p class="flex max-w-md items-center gap-2" aria-hidden="true"><Spinner v-if="session.banner?.action !== 'reload'" :size="12" />{{ session.banner && said(session.banner) }}</p>
      <Button v-if="session.banner?.action === 'reload'" size="sm" data-reload @click="reload"><Icon name="repeat" :size="12" /> Reload</Button>
      <Button v-else-if="session.banner?.action === 'tryNow'" size="sm" variant="outline" data-try-now @click="ask({ ask: 'connect' })"><Icon name="repeat" :size="12" /> Try now</Button>
    </div>
    <div v-else class="flex h-full flex-col">
      <!-- Out of sight until the keyboard reaches it; its target takes the focus without joining the tab order. -->
      <a href="#main" class="sr-only focus:not-sr-only focus:fixed focus:top-2 focus:left-2 focus:z-50 focus:rounded-md focus:border focus:border-line focus:bg-panel focus:px-3 focus:py-1.5 focus:text-text" @click.prevent="main?.focus()">Skip to content</a>
      <TopBar :to-review="toReview" @find="finding = true" />
      <ConnectionBanner />
      <HealthBar />
      <ResizablePanelGroup :direction="stacked ? 'vertical' : 'horizontal'" :auto-save-id="stacked ? 'shell-stacked' : 'shell'" class="min-h-0 flex-1">
        <ResizablePanel id="shell-view" :order="1" class="flex" :min-size="MAIN_MIN_PERCENT">
          <main id="main" ref="main" tabindex="-1" class="flex min-h-0 min-w-0 flex-1 flex-col outline-none" :aria-busy="catalog.loaded ? undefined : true">
            <h1 v-if="page" class="sr-only">{{ page }}</h1>
            <RouterView v-if="catalog.loaded" />
            <PageBody v-else><EmptyState class="flex items-center gap-2"><Spinner :size="12" />Loading…</EmptyState></PageBody>
          </main>
        </ResizablePanel>
        <template v-if="showsAssistant">
          <ResizableHandle aria-label="Resize the Assistant" />
          <ResizablePanel id="shell-assistant" :order="2" class="flex" v-bind="stacked ? stackedAssistant : assistantColumn"><AssistantPanel :view="assistant.view!" /></ResizablePanel>
        </template>
      </ResizablePanelGroup>
    </div>
    <div role="status" class="sr-only">{{ waits }}</div>
    <NewChain v-if="creating && !offline" @close="closeNew()" />
    <CommandPalette v-if="!blank" v-model:open="finding" />
    <KeysHelp v-if="!blank" v-model:open="listingKeys" />
    <Toaster :duration="2800" position="bottom-center" />
  </TooltipProvider>
</template>
