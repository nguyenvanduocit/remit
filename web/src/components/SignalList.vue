<script setup lang="ts">
import { useRoute } from 'vue-router';
import type { SignalLine, SignalSourceLine } from '../core/generated/types.generated';
import { fullTime, plural, timed } from '../ui';
import Ago from './Ago.vue';
import EmptyState from './EmptyState.vue';
import Icon from './Icon.vue';
import ItemCard from './ItemCard.vue';
import ShowMore from './ShowMore.vue';
import { Badge } from '@/components/ui/badge';

// The Signals list: every signal the board lists, the newest reading first, a row each (its key, its subscriptions, its newest
// reading), a page at a time (more shows the next). The page shows it whole; an open signal's page shows it beside it (openId),
// marked. With a key shape the server knows typed (picked), the list opens with what its signals say and who may listen, and one
// with no signal says why: time keeps none (a subscription keeps its own schedule). A row keeps the search.
defineProps<{ rows: readonly SignalLine[]; count: number; hidden: number; unlisted: number; picked?: SignalSourceLine; openId?: string }>();
defineEmits<{ more: [] }>();
const route = useRoute();
const to = (entry: string) => ({ name: 'signals', params: { entry }, query: { q: route.query.q } });
</script>

<template>
  <section class="flex min-h-0 flex-col" aria-label="Signals" data-list>
    <h2 class="caps flex items-center gap-[7px] px-1.5 pt-1 pb-2.5">
      <Icon name="list" :size="12" />Signals <Badge variant="quiet" size="count" class="ml-auto tracking-normal" data-count>{{ count }}</Badge>
    </h2>
    <div v-if="picked" class="px-1.5 pb-2.5 text-xs text-mut" data-picked>
      <p class="line-clamp-3 [overflow-wrap:anywhere]" :title="picked.what" data-what>{{ picked.what }}</p>
      <p class="mt-1 line-clamp-2 [overflow-wrap:anywhere]" :title="picked.who" data-who>Who listens: {{ picked.who }}</p>
    </div>
    <div class="flex flex-1 flex-col gap-2 overflow-auto">
      <ItemCard v-for="s in rows" :key="s.id" :to="to(s.id)" :on="openId === s.id" :title="s.key" mono :data-entry="s.id">
        <template #badge>
          <span v-if="s.silent" class="inline-flex items-center gap-1 text-warning" data-silent><Icon name="alert" :size="11" />Silent</span>
          <span v-else-if="!s.rev" data-unread>No reading yet</span>
        </template>
        <template #meta>
          <span data-subscribers>{{ plural(s.subscribers, 'subscription') }}</span>
          <span v-if="s.rev" class="ml-auto tabular-nums" :title="timed(s.at) ? fullTime(s.at!) : undefined" data-read>rev {{ s.rev }}<template v-if="timed(s.at)"> · <Ago :at="s.at!" /></template></span>
        </template>
      </ItemCard>
      <ShowMore v-if="hidden > 0" @click="$emit('more')">Show {{ Math.min(30, hidden) }} more of {{ hidden }}</ShowMore>
      <p v-if="unlisted && !hidden" class="px-1.5 text-xs text-mut" data-unlisted>{{ unlisted }} older not listed: the list holds the newest signals.</p>
      <EmptyState v-if="!count" class="px-1.5 py-4 text-center text-xs" data-empty>
        <template v-if="picked?.source === 'clock'">No signal is kept for time: each subscription keeps its own schedule, and its firings are its rows.</template>
        <template v-else-if="route.query.q && !picked">No signal matches the search.</template>
        <template v-else>No signal yet: one shows here once it has a reading or a listener.</template>
      </EmptyState>
    </div>
  </section>
</template>
