<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import { catalog } from '../catalog';
import { useColumnSlide } from '../columns';
import { feedbackKindKeys, useFeedbackColumns } from '../feedbackColumns';
import type { FeedbackKind } from '../core/generated/types.generated';
import ColumnArrows from './ColumnArrows.vue';
import ColumnSidebar from './ColumnSidebar.vue';
import FeedbackColumn from './FeedbackColumn.vue';
import SearchBox from './SearchBox.vue';

// Beside the open feedback, the Feedback board one column at a time: the column of its kind, unless the owner slid to another
// (?col=). The way back to the whole board is Esc on the page, or its entry in the top bar.
const route = useRoute();
const openId = computed(() => (typeof route.params.id === 'string' ? route.params.id : ''));
const own = computed<FeedbackKind | undefined>(() => catalog.feedback.find((f) => f.id === openId.value)?.kind);
const { shownKey, at, go, slide } = useColumnSlide<FeedbackKind>(feedbackKindKeys, () => own.value);
const { columns, query, more } = useFeedbackColumns();
const shown = computed(() => columns.value[at.value]);
const prev = computed(() => columns.value[at.value - 1]);
const next = computed(() => columns.value[at.value + 1]);
</script>

<template>
  <ColumnSidebar :columns="columns" :shown-key="shownKey" :slide="slide" label="Feedback columns" @go="go($event as FeedbackKind)">
    <template #head>
      <SearchBox v-model="query" placeholder="Search feedback…" label="Search feedback" />
    </template>
    <FeedbackColumn v-if="shown" :key="shown.key" class="min-w-0 flex-1" :column="shown" :open-id="openId" @more="more(shown.key)">
      <template #nav><ColumnArrows :prev="prev" :next="next" @go="go($event as FeedbackKind)" /></template>
    </FeedbackColumn>
  </ColumnSidebar>
</template>
