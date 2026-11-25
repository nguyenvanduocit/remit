<script setup lang="ts">
import type { SignalSourceLine } from '../core/generated/types.generated';
import { patternOf } from '../signalList';
import Icon from './Icon.vue';
import SearchBox from './SearchBox.vue';
import { Button } from '@/components/ui/button';
import { DropdownMenu, DropdownMenuContent, DropdownMenuRadioGroup, DropdownMenuRadioItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu';

// What narrows the Signals list, over the whole list and beside an open signal alike: the search, and beside it the key shapes the
// server knows, each a pattern that fills the search when picked (signalList.ts), named and counted.
defineProps<{ sources: readonly SignalSourceLine[]; total: number }>();
const query = defineModel<string>('query', { required: true });
const pick = (v: unknown) => {
  if (typeof v === 'string') query.value = v;
};
</script>

<template>
  <div class="flex gap-1.5">
    <SearchBox v-model="query" size="sm" class="min-w-0 flex-1" placeholder="Search signals or a key pattern (fact:*)…" label="Search signals" />
    <DropdownMenu>
      <DropdownMenuTrigger as-child>
        <Button class="flex-none" variant="outline" size="sm" aria-label="Key patterns" data-patterns><Icon name="filter" :size="13" /></Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" class="w-72">
        <DropdownMenuRadioGroup :model-value="query.trim()" @update:model-value="pick">
          <DropdownMenuRadioItem value="">All signals <span class="ml-auto text-muted-foreground tabular-nums">{{ total }}</span></DropdownMenuRadioItem>
          <DropdownMenuRadioItem v-for="s in sources" :key="s.id" :value="patternOf(s)" :data-pattern-option="patternOf(s)">
            <span class="font-mono text-[12px]">{{ patternOf(s) }}</span><span class="text-muted-foreground">{{ s.title }}</span>
            <span class="ml-auto text-muted-foreground tabular-nums">{{ s.signals }}</span>
          </DropdownMenuRadioItem>
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  </div>
</template>
