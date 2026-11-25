<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import { archivedChains } from '../archived';
import { columnKeys, columnOf, useBoard, type ColumnKey } from '../board';
import { catalog, listedChain } from '../catalog';
import { useColumnSlide } from '../columns';
import { opened } from '../openChain';
import BoardColumn from './BoardColumn.vue';
import ColumnArrows from './ColumnArrows.vue';
import ColumnSidebar from './ColumnSidebar.vue';
import ProjectFilter from './ProjectFilter.vue';
import SearchBox from './SearchBox.vue';

// Beside the open chain, the board one column at a time (ColumnSidebar, useColumnSlide): the column that holds the chain, so
// it follows the chain as it moves (from Running to Waiting for you), unless the owner slid to another (?col=), narrowed as the
// board was by the search and the project.
const route = useRoute();
const openId = computed(() => (typeof route.params.id === 'string' ? route.params.id : ''));
const own = computed<ColumnKey | undefined>(() => {
  const id = openId.value;
  const chain = opened.view?.chain.chain.id === id ? opened.view.chain.chain : (listedChain(id) ?? archivedChains.value.find((c) => c.id === id));
  return chain && columnOf(catalog.board, chain);
});
const { shownKey, at, go, slide } = useColumnSlide<ColumnKey>(() => columnKeys(catalog.board), () => own.value);
const { columns, query, project, more } = useBoard(() => shownKey.value === 'archived');
// Shown once the board and the chain's place on it are known (or the chain is loaded and on no column yet, just created), so
// the first column shown is the chain's own and nothing slides as the app loads.
const known = computed(() => !!catalog.board && (!!own.value || opened.view?.chain.chain.id === openId.value));
const shown = computed(() => (known.value ? columns.value[at.value] : undefined));
const prev = computed(() => columns.value[at.value - 1]);
const next = computed(() => columns.value[at.value + 1]);
</script>

<template>
  <ColumnSidebar :columns="columns" :shown-key="shownKey" :slide="slide" label="Board columns" @go="go($event as ColumnKey)">
    <template #head><div class="flex gap-1.5"><SearchBox v-model="query" class="min-w-0 flex-1" placeholder="Search chains…" label="Search chains" /><ProjectFilter v-model="project" size="default" /></div></template>
    <BoardColumn v-if="shown" :key="shown.key" class="min-w-0 flex-1" :column="shown" :open-id="openId" @more="more(shown.key)">
      <template #nav><ColumnArrows :prev="prev" :next="next" @go="go($event as ColumnKey)" /></template>
    </BoardColumn>
  </ColumnSidebar>
</template>
