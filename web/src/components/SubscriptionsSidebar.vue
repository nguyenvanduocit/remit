<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import { catalog } from '../catalog';
import { useColumnSlide } from '../columns';
import { subscriptionColumnKeys, useSubscriptionColumns } from '../subscriptionColumns';
import type { SubscriptionGroup } from '../core/generated/types.generated';
import ColumnArrows from './ColumnArrows.vue';
import ColumnSidebar from './ColumnSidebar.vue';
import SignalsFilter from './SignalsFilter.vue';
import SubscriptionColumn from './SubscriptionColumn.vue';

// Beside the open subscription, the Subscriptions board one column at a time: the column that holds the subscription, so
// it follows it as it moves (from waiting to held), unless the owner slid to another (?col=). The way back to the whole board is Esc on the page, or its entry in the top bar.
const route = useRoute();
const openId = computed(() => (typeof route.params.entry === 'string' ? route.params.entry : ''));
const own = computed<SubscriptionGroup | undefined>(() => catalog.signals?.subscriptions.find((l) => l.id === openId.value)?.group);
const { shownKey, at, go, slide } = useColumnSlide<SubscriptionGroup>(subscriptionColumnKeys, () => own.value);
const { columns, query, kinds, kindCounts, more } = useSubscriptionColumns();
const shown = computed(() => columns.value[at.value]);
const prev = computed(() => columns.value[at.value - 1]);
const next = computed(() => columns.value[at.value + 1]);
</script>

<template>
  <ColumnSidebar :columns="columns" :shown-key="shownKey" :slide="slide" label="Signal columns" @go="go($event as SubscriptionGroup)">
    <template #head>
      <SignalsFilter v-model:query="query" v-model:kinds="kinds" :kind-counts="kindCounts" />
    </template>
    <SubscriptionColumn v-if="shown" :key="shown.key" class="min-w-0 flex-1" :column="shown" :open-id="openId" @more="more(shown.key)">
      <template #nav><ColumnArrows :prev="prev" :next="next" @go="go($event as SubscriptionGroup)" /></template>
    </SubscriptionColumn>
  </ColumnSidebar>
</template>
