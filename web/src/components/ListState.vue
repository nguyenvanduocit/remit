<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue';
import EmptyState from './EmptyState.vue';
import Icon from './Icon.vue';
import Spinner from './Spinner.vue';
import { Button } from '@/components/ui/button';

// Every list the screen asks the server for (useRemote) in one grammar, the same words and the same line wherever it shows:
// while nothing is held yet, "Reading <what>…" after 150 ms (a quick answer never blinks); when that fails, "Could not read
// <what>: <why>." and Try again; when what is held could not be read again, it stays on screen under one line saying so
// (partial), with Try again; held and empty, the `empty` slot, which says what fills the list and who; else the list itself
// (the default slot). `what` names it as it reads after "Reading": "the telemetry", "where duels stand"; `retry` (useRemote's)
// is what Try again does, and with none there is no Try again. Its parent places it: it draws no box of its own.
const props = defineProps<{ what: string; held: boolean; error?: string; empty?: boolean; retry?: () => void }>();

// The loading line shows only once the read has taken 150 ms.
const late = ref(false);
let timer: ReturnType<typeof setTimeout> | undefined;
watch(
  () => !props.held && !props.error,
  (waiting) => {
    clearTimeout(timer);
    late.value = false;
    if (waiting) timer = setTimeout(() => (late.value = true), 150);
  },
  { immediate: true },
);
onBeforeUnmount(() => clearTimeout(timer));
</script>

<template>
  <div v-if="error && !held" class="flex items-start gap-2" role="alert" data-list-state="error">
    <Icon class="mt-[3px] flex-none text-red-text" name="alert" :size="13" />
    <div class="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
      <span class="min-w-0 text-red-text [overflow-wrap:anywhere]">Could not read {{ what }}: {{ error }}.</span>
      <Button v-if="retry" variant="outline" size="xs" data-retry @click="retry">Try again</Button>
    </div>
  </div>
  <EmptyState v-else-if="!held" class="flex items-center gap-2" role="status" :aria-busy="true" data-list-state="loading"><template v-if="late"><Spinner :size="12" />Reading {{ what }}…</template></EmptyState>
  <template v-else>
    <div v-if="error" class="mb-2 flex items-start gap-2 text-[12px] text-text2" role="alert" data-list-state="partial">
      <Icon class="mt-[2px] flex-none text-warning" name="alert" :size="12" />
      <div class="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
        <span class="min-w-0 [overflow-wrap:anywhere]">Showing what was read before: could not read {{ what }} again ({{ error }}).</span>
        <Button v-if="retry" variant="outline" size="xs" data-retry @click="retry">Try again</Button>
      </div>
    </div>
    <EmptyState v-if="empty" data-list-state="empty"><slot name="empty" /></EmptyState>
    <slot v-else />
  </template>
</template>
