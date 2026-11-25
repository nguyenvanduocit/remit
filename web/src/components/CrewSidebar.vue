<script setup lang="ts" generic="T">
import { computed } from 'vue';
import { useColumnSlide } from '../columns';
import { useCrewColumns, type CrewBoard } from '../crewColumns';
import ColumnArrows from './ColumnArrows.vue';
import ColumnSidebar from './ColumnSidebar.vue';
import CrewColumn from './CrewColumn.vue';
import SearchBox from './SearchBox.vue';

// Beside the open item of a Crew page, its board one column at a time (ColumnSidebar, useColumnSlide): the column of the
// item's own group, unless the owner slid to another (?col=). The way back to the whole board is Esc on the page, or the
// page's tab. `noun` names what is listed ("playbooks") for the search and the marks.
const props = defineProps<{ board: CrewBoard<T>; openId: string; noun: string }>();
defineSlots<{ row(props: { row: T; on: boolean }): unknown }>();
const { columns, keys, query, more } = useCrewColumns(props.board);
const own = computed(() => {
  const open = props.board.rows().find((r) => props.board.keyOf(r) === props.openId);
  return open && props.board.groupOf(open);
});
const { shownKey, at, go, slide } = useColumnSlide<string>(() => keys.value, () => own.value);
const shown = computed(() => columns.value[at.value]);
const prev = computed(() => columns.value[at.value - 1]);
const next = computed(() => columns.value[at.value + 1]);
</script>

<template>
  <!-- A page with no group at all (nothing listed yet) has no column to show. -->
  <ColumnSidebar v-if="columns.length" :columns="columns" :shown-key="shownKey" :slide="slide" :label="`Columns of ${noun}`" @go="go($event)">
    <template #head><SearchBox v-model="query" :placeholder="`Search ${noun}…`" :label="`Search ${noun}`" /></template>
    <CrewColumn v-if="shown" :key="shown.key" class="min-w-0 flex-1" :column="shown" :key-of="board.keyOf" :open-id="openId" @more="more(shown.key)">
      <template #row="p"><slot name="row" v-bind="p" /></template>
      <template #nav><ColumnArrows :prev="prev" :next="next" @go="go($event)" /></template>
    </CrewColumn>
  </ColumnSidebar>
</template>
