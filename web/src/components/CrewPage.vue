<script setup lang="ts" generic="T">
import { useRoute, useRouter } from 'vue-router';
import { useCrewColumns, type CrewBoard } from '../crewColumns';
import type { Area } from '../router';
import AuthoringChains from './AuthoringChains.vue';
import CrewColumn from './CrewColumn.vue';
import CrewSidebar from './CrewSidebar.vue';
import ListPage from './ListPage.vue';
import PageTabs from './PageTabs.vue';
import SearchBox from './SearchBox.vue';

// A page of what the owner's agents are made of (playbooks, agents, skills) and the tools they read through, under the tabs of
// its area (`group`: Crew, or Integrations for the tools), in the board's shape. With nothing open, the page is the board: the search,
// then every item as a card in one column per group its record carries (crewColumns.ts), side by side, a page at a time; with
// none yet, what the page says instead (the `empty` slot: an EmptyPage). A card opens its item: the address names it
// (`openId`), the board folds to its column beside it (CrewSidebar, ?col=), and the page is the item's, under the header every
// page about one thing has (ListPage: `title`, `standing`, `actions`), as the default slot draws it; an address that
// names nothing listed says so (`gone`). Esc goes back to the whole board, keeping the search; so does the page's tab. The
// author chains still at work stay under both. Nothing is edited here: the owner tells the Assistant, and the
// lists change as the author saves. `noun` names what is listed ("playbooks") for the search and the marks.
const props = defineProps<{ group: Area; name: string; board: CrewBoard<T>; openId: string; open: boolean; noun: string; title?: string; mono?: boolean; gone: string }>();
defineSlots<{ row(props: { row: T; on: boolean }): unknown; empty(): unknown; standing(): unknown; actions(): unknown; default(): unknown }>();
const route = useRoute();
const router = useRouter();
const { columns, query, total, more } = useCrewColumns(props.board);
// Esc on the open item's page goes back to the whole board; in a search with words in it, Esc only clears them (SearchBox), so
// the page looks at the key before the search does (capture), while the words are still there.
const back = () => void router.push({ name: props.name, query: { q: route.query.q } });
const onEsc = (e: KeyboardEvent) => {
  if (e.target instanceof HTMLInputElement && e.target.value) return;
  back();
};
</script>

<template>
  <section v-if="!openId" class="flex min-h-0 flex-1 flex-col overflow-hidden" data-board>
    <PageTabs :group="group">
      <template v-if="total" #end>
        <SearchBox v-model="query" size="sm" class="w-[320px] max-w-full flex-none" :placeholder="`Search ${noun}…`" :label="`Search ${noun}`" />
      </template>
    </PageTabs>
    <div v-if="!total" class="flex min-h-0 min-w-0 flex-1 flex-col"><slot name="empty" /></div>
    <template v-else>
      <div class="grid min-h-0 flex-1 auto-cols-[minmax(240px,1fr)] grid-flow-col gap-2.5 overflow-x-auto px-4 pt-2.5 pb-3.5">
        <CrewColumn v-for="col in columns" :key="col.key" class="rounded-lg bg-bg2 p-2" :column="col" :key-of="board.keyOf" @more="more(col.key)">
          <template #row="p"><slot name="row" v-bind="p" /></template>
        </CrewColumn>
      </div>
    </template>
    <AuthoringChains />
  </section>
  <ListPage v-else :name="name" :open="open" :title="title" :mono="mono" :empty="gone" @keydown.esc.capture="onEsc">
    <template #tabs><PageTabs :group="group" /></template>
    <template #sidebar>
      <CrewSidebar :board="board" :open-id="openId" :noun="noun">
        <template #row="p"><slot name="row" v-bind="p" /></template>
      </CrewSidebar>
    </template>
    <template #standing><slot name="standing" /></template>
    <template #actions><slot name="actions" /></template>
    <slot />
    <template #foot><AuthoringChains /></template>
  </ListPage>
</template>
