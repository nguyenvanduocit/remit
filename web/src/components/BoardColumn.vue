<script setup lang="ts">
import { computed } from 'vue';
import Icon from './Icon.vue';
import type { ShownColumn } from '../board';
import { useClock } from '../clock';
import { span } from '../ui';
import { oldestSince } from '../waits';
import ChainCard from './ChainCard.vue';
import EmptyState from './EmptyState.vue';
import InboxCallout from './InboxCallout.vue';
import Kbd from './Kbd.vue';
import Tip from './Tip.vue';
import { Badge } from '@/components/ui/badge';
import ShowMore from './ShowMore.vue';

// One column of the board: its name (a heading, so a screen reader jumps between columns) and how many it holds, what waits for the owner from no chain (in the first), then its
// chains in the server's order, a page at a time (more asks for the next). The column of what needs the owner says how long
// the oldest of it has waited, and that `n` opens the next (the word itself only where the column has room for it). The board shows its columns side by side; the open chain's page
// shows one beside the chain (openId), marked, and adds a way to the other columns beside the count (#nav).
const props = defineProps<{ column: ShownColumn; openId?: string }>();
defineEmits<{ more: [] }>();
const oldest = computed(() => (props.column.key === 'needs' ? oldestSince.value : undefined));
const now = useClock();
const waited = computed(() => (oldest.value ? span(now.value.getTime() - new Date(oldest.value).getTime()) : ''));
</script>

<template>
  <section class="@container flex min-h-0 flex-col" :aria-label="column.label">
    <div class="caps flex items-center gap-[7px] px-1.5 pt-1 pb-2.5">
      <h2 class="flex flex-none items-center gap-[7px]"><Icon class="text-blue-mark" name="spark" :size="12" />{{ column.label }}</h2>
      <Tip v-if="oldest" text="How long the oldest has waited. Press n to open the next."><span class="oldest ml-auto flex min-w-0 items-center gap-1 overflow-hidden whitespace-nowrap" data-oldest><Icon class="flex-none" name="clock" :size="11" /><span class="sr-only">oldest</span>{{ waited }} · <Kbd>n</Kbd><span class="@max-[250px]:sr-only">next</span></span></Tip>
      <Badge variant="quiet" size="count" class="flex-none tracking-normal" :class="{ 'ml-auto': !oldest }">{{ column.count }}</Badge><slot name="nav" />
    </div>
    <div class="flex flex-1 flex-col gap-2 overflow-auto" data-rows>
      <InboxCallout v-for="i in column.items" :key="i.id" :item="i" />
      <ChainCard v-for="c in column.chains" :key="c.id" :chain="c" :finished="column.key === 'finished' || column.key === 'archived'" :on="openId === c.id" />
      <ShowMore v-if="column.hidden > 0" @click="$emit('more')">Show {{ column.hidden }} more</ShowMore>
      <!-- Centred in the column, under its name: the column's own width is where the eye looks for its cards. -->
      <EmptyState v-if="!column.count" class="px-2 py-4 text-center text-xs" data-column-empty>{{ column.empty }}</EmptyState>
    </div>
  </section>
</template>

<style scoped>
/* The column's head is in capitals; how long the oldest has waited is a note beside it, in the line's own case. */
.oldest {
  font-size: 11px;
  letter-spacing: normal;
  text-transform: none;
}
</style>
