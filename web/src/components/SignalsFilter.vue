<script setup lang="ts">
import { computed } from 'vue';
import { kindQueryOf, kindStateOf, kindSummary, nextKindState, subscriptionKinds, type KindState, type KindStates } from '../subscriptionColumns';
import type { SubscriptionKind } from '../core/generated/types.generated';
import Icon from './Icon.vue';
import SearchBox from './SearchBox.vue';
import { Button } from '@/components/ui/button';
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu';

// What narrows the Signals board, over its columns and beside an open subscription alike: the search, and a menu of the kinds,
// each counted, where a click moves a kind from any to only to hidden and back (the button says what shows while it is not as
// it starts). The menu stays open while the owner clicks through it.
defineProps<{ kindCounts: Partial<Record<SubscriptionKind, number>> }>();
const query = defineModel<string>('query', { required: true });
const kinds = defineModel<KindStates>('kinds', { required: true });

const STATE_WORD: Record<KindState, string> = { any: 'any', only: 'only', hide: 'hidden' };
const STATE_ICON: Record<KindState, string> = { any: '', only: 'check', hide: 'x' };
const said = computed(() => subscriptionKinds());
const changed = computed(() => {
  const q = kindQueryOf(said.value, kinds.value);
  return q.kind !== undefined || q.hide !== undefined;
});
const summary = computed(() => kindSummary(said.value, kinds.value));
const stateOf = (k: SubscriptionKind) => kindStateOf(kinds.value, k);
const step = (k: SubscriptionKind) => (kinds.value = { ...kinds.value, [k]: nextKindState(stateOf(k)) });
</script>

<template>
  <div class="flex gap-1.5">
    <SearchBox v-model="query" size="sm" class="min-w-0 flex-1" placeholder="Search subscriptions…" label="Search subscriptions" />
    <DropdownMenu>
      <DropdownMenuTrigger as-child>
        <Button class="flex-none" :variant="changed ? 'secondary' : 'outline'" size="sm" :aria-label="`Filter by kind: ${summary}`" data-kind-filter>
          <Icon name="filter" :size="13" /><span v-if="changed" class="max-w-28 truncate">{{ summary }}</span>
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" class="w-60">
        <DropdownMenuItem v-for="k in said" :key="k.kind" class="relative pr-8" :aria-label="`${k.plural}: ${STATE_WORD[stateOf(k.kind)]}`" :data-kind-state="k.kind" :data-filter-state="stateOf(k.kind)" @select.prevent="step(k.kind)">
          <span :class="stateOf(k.kind) === 'hide' && 'text-muted-foreground line-through'">{{ k.plural }}</span>
          <span class="ml-auto text-muted-foreground tabular-nums">{{ kindCounts[k.kind] ?? 0 }}</span>
          <span class="pointer-events-none absolute right-2 flex items-center justify-center" :class="stateOf(k.kind) === 'hide' ? 'text-muted-foreground' : 'text-green-text'"><Icon v-if="STATE_ICON[stateOf(k.kind)]" :name="STATE_ICON[stateOf(k.kind)]" :size="13" /></span>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  </div>
</template>
