<script setup lang="ts">
import { computed, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import BoardSidebar from './BoardSidebar.vue';
import ChainPage from './ChainPage.vue';
import EmptyState from './EmptyState.vue';
import Icon from './Icon.vue';
import ListPage from './ListPage.vue';
import PageBody from './PageBody.vue';
import Spinner from './Spinner.vue';
import CardDrawer from './CardDrawer.vue';
import { catalog, listed } from '../catalog';
import { chairOf, opened, openChain } from '../openChain';
import { session } from '../session';
import { defaultTab, onScreen, useSelected } from '../nav';
import type { Chain } from '../core/generated/types.generated';

// An open chain: the board's column that holds it, the chain's page (until it is loaded, that it opens) and the drawer of
// the agent picked in it. The address is the only thing that opens a chain: /chains/<id> opens that one, and a chain
// without a tab gets its default one, the Overview.
const route = useRoute();
const router = useRouter();
const routeId = computed(() => (typeof route.params.id === 'string' ? route.params.id : undefined));
const settle = (c: Pick<Chain, 'id'>) => router.replace({ name: 'chain', params: { id: c.id, tab: defaultTab }, query: route.query });

watch(
  routeId,
  (id) => {
    if (!id) return;
    if (opened.view?.chain.chain.id === id || opened.opening === id) return;
    openChain(id);
  },
  { immediate: true },
);
// The chain the address names could not be opened (the core says why) and the server does not list it: it is gone, and the
// address moves on to the board. One it lists that could not be loaded now (the connection dropped, no answer in time)
// keeps its address: the core opens it again when the connection is back.
watch(
  () => {
    const id = routeId.value;
    return !!id && catalog.loaded && session.link.phase === 'live' && !opened.opening && opened.view?.chain.chain.id !== id && !listed(id);
  },
  (gone) => gone && void router.replace({ name: 'board', query: route.query }),
);
// The tab, once the chain the address names is known.
watch(
  () => [routeId.value, route.params.tab, opened.view?.chain.chain] as const,
  ([id, tab, open]) => {
    if (id && !tab && open?.id === id) void settle(open);
  },
  { immediate: true },
);
// The chain the address names was open and is closed: the server deleted it (the core closes it), so the address moves
// on to the board. Being listed does not say it: an archived chain never is, and opening one leaves a listed chain.
watch(
  () => opened.view?.chain.chain.id,
  (open, was) => {
    if (was && !open && was === routeId.value) void router.replace({ name: 'board', query: route.query });
  },
);

// What is on screen of the chain the address names; while another is on its way, the old one steps back (ListPage busy). With no
// old one on screen there is nothing to step back, and the line that says the chain is opening is read at full strength.
const screen = computed(() => onScreen(opened.view?.chain.chain.id, routeId.value ?? '', opened.opening));
const view = computed(() => (screen.value.shows ? opened.view : null));

const selected = useSelected();
const selectedCard = computed(() => {
  const v = view.value;
  if (!v) return undefined;
  return (selected.value !== 'chair' && v.cards.find((c) => c.id === selected.value)) || chairOf(v);
});
</script>

<template>
  <ListPage name="chains" :open="!!routeId" :busy="screen.shows && screen.switching">
    <template #sidebar><BoardSidebar /></template>
    <ChainPage v-if="view" :view="view" />
    <PageBody v-else>
      <EmptyState v-if="opened.opening" role="status" class="flex items-center gap-2"><Spinner :size="12" />Opening the chain…</EmptyState>
      <EmptyState v-else role="status" class="flex items-center gap-2"><Icon name="alert" :size="13" />This chain could not be opened. It opens once the connection is back.</EmptyState>
    </PageBody>
    <template #drawer>
      <CardDrawer v-if="view && selectedCard" :view="view" :card="selectedCard" />
      <div v-else-if="routeId" class="min-w-0 flex-1 px-5 py-4"><EmptyState data-drawer-empty>Its agents show here once the chain is open.</EmptyState></div>
    </template>
  </ListPage>
</template>
