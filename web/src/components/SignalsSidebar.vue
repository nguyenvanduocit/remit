<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import { useSignalList } from '../signalList';
import SignalList from './SignalList.vue';
import SignalSearch from './SignalSearch.vue';
import Sidebar from './Sidebar.vue';

// Beside the open signal, the Signals list with its search: the one open is marked. The way back to the whole list is Esc on the
// page, or its tab.
const route = useRoute();
const openId = computed(() => (typeof route.params.entry === 'string' ? route.params.entry : ''));
const { rows, count, hidden, unlisted, query, picked, sources, total, more } = useSignalList();
</script>

<template>
  <Sidebar>
    <template #head><SignalSearch v-model:query="query" :sources="sources" :total="total" /></template>
    <SignalList class="min-w-0 flex-1" :rows="rows" :count="count" :hidden="hidden" :unlisted="unlisted" :picked="picked" :open-id="openId" @more="more" />
  </Sidebar>
</template>
