<script setup lang="ts">
import { computed } from 'vue';
import { useApi } from '../core/api';
import { useRemote } from '../useRemote';
import MemoryBook from './MemoryBook.vue';

// What an agent remembers, in its drawer's Memory panel: its employee's memory book, or the Assistant's (`card_memory`), read
// as the panel opens. No push carries a memory, so one learned while it is open shows when it is opened again. `viewKey` is
// the address's key for the book's open tab, the drawer's own. `employee` is the employee the agent is, empty for the
// Assistant: only an employee's memory may be forgotten.
const props = defineProps<{ cardId: string; name: string; viewKey: string; employee: string }>();
const api = useApi();
const memory = useRemote(
  () => ({ id: props.cardId }),
  (asked) => api.cardMemory(asked.id),
);
const book = computed(() => (memory.of.value?.id === props.cardId ? memory.data.value : undefined));
</script>

<template>
  <div data-agent-memory>
    <MemoryBook :book="book" :error="memory.error.value" :loading="memory.loading.value && memory.of.value?.id !== cardId" :who="name" :view-key="viewKey" surface="bg2" :employee="employee || undefined" :retry="memory.retry" />
  </div>
</template>
