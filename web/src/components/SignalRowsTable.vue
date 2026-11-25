<script setup lang="ts">
import type { SignalRow } from '../core/generated/types.generated';
import Ago from './Ago.vue';
import EmptyState from './EmptyState.vue';
import ShowMore from './ShowMore.vue';
import Tip from './Tip.vue';
import { Table, TableBody, TableCell, TableEmpty, TableHead, TableHeader, TableRow } from '@/components/ui/table';

// The Signals table: one line per stored row, a reading or a firing, newest first (signalRows.ts). The key opens its signal; a
// reading of an event opens what became of it, a firing the subscription that fired.
defineProps<{ rows: readonly SignalRow[]; status: 'loading' | 'ready' | 'failed'; full: boolean }>();
defineEmits<{ more: [] }>();
const at = (r: SignalRow) => r.reading?.at ?? r.firing?.at ?? '';
const id = (r: SignalRow) => (r.reading ? `r:${r.key}#${r.reading.rev}` : `f:${r.firing?.subscriptionId}:${r.key}:${r.firing?.at}`);
const outcome = (f: NonNullable<SignalRow['firing']>) => (f.expired ? 'its deadline came with no reading' : f.outcome);
</script>

<template>
  <div class="flex min-h-0 flex-col" data-rows>
    <Table class="text-[12.5px]">
      <TableHeader>
        <TableRow><TableHead class="w-28 pl-4">At</TableHead><TableHead class="w-20">Kind</TableHead><TableHead class="w-[34%]">Key</TableHead><TableHead class="w-14 text-right">Rev</TableHead><TableHead class="pr-4">What</TableHead></TableRow>
      </TableHeader>
      <TableBody>
        <TableEmpty v-if="status === 'ready' && !rows.length" :colspan="5" class="text-mut" data-empty>No reading or firing has a key that matches.</TableEmpty>
        <TableRow v-for="r in rows" :key="id(r)" :data-row="r.reading ? 'reading' : 'firing'" :data-key="r.key">
          <TableCell class="pl-4 whitespace-nowrap text-mut tabular-nums"><Ago :at="at(r)" /></TableCell>
          <TableCell class="text-text2">{{ r.reading ? 'reading' : 'firing' }}</TableCell>
          <TableCell class="max-w-0 font-mono text-[12px]">
            <Tip :text="r.key || 'clock'" when-clipped>
              <span class="block truncate">
                <RouterLink v-if="r.key && !r.key.startsWith('clock')" class="hover:underline" :to="{ name: 'signals', params: { entry: `signal:${r.key}` } }" data-key-link>{{ r.key }}</RouterLink>
                <template v-else>{{ r.key || 'clock' }}</template>
              </span>
            </Tip>
          </TableCell>
          <TableCell class="text-right text-mut tabular-nums">{{ r.reading?.rev ?? '' }}</TableCell>
          <TableCell class="max-w-0 pr-4">
            <span v-if="r.reading" class="flex min-w-0 items-center gap-2" data-what>
              <RouterLink v-if="r.reading.eventId" class="flex-none text-blue-text hover:underline" :to="{ name: 'events', params: { id: r.reading.eventId } }" data-event-link>event</RouterLink>
              <Tip :text="r.reading.value" when-clipped><span class="truncate font-mono text-[12px] text-text2">{{ r.reading.value }}</span></Tip>
            </span>
            <span v-else-if="r.firing" class="flex min-w-0 items-center gap-2" data-what>
              <RouterLink v-if="r.firing.subscriptionId" class="flex-none text-blue-text hover:underline" :to="{ name: 'subscriptions', params: { entry: r.firing.subscriptionId } }" data-subscription-link>{{ r.firing.title }}</RouterLink>
              <span v-else class="flex-none">{{ r.firing.title }}</span>
              <Tip :text="outcome(r.firing)" when-clipped><span class="truncate text-text2">{{ outcome(r.firing) }}</span></Tip>
            </span>
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
    <ShowMore v-if="full" class="mt-2" @click="$emit('more')">Show more</ShowMore>
    <EmptyState v-if="status === 'failed'" class="px-4 py-3 text-xs" data-failed>The rows could not be read: they are read again when the link is live.</EmptyState>
  </div>
</template>
