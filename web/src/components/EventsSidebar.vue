<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import { useEventList } from '../eventList';
import EventList from './EventList.vue';
import SearchBox from './SearchBox.vue';
import Sidebar from './Sidebar.vue';

// Beside the open event, the Events list: the event open is marked. The way back to the whole page is Esc on it, or its entry in
// the top bar.
const route = useRoute();
const openId = computed(() => (typeof route.params.id === 'string' ? route.params.id : ''));
const { rows, count, hidden, query, more } = useEventList();
</script>

<template>
  <Sidebar>
    <template #head><SearchBox v-model="query" placeholder="Search events…" label="Search events" /></template>
    <EventList class="min-w-0 flex-1" :rows="rows" :count="count" :hidden="hidden" :open-id="openId" @more="more" />
  </Sidebar>
</template>
