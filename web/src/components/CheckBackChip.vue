<script setup lang="ts">
import Icon from './Icon.vue';
import { clockAt } from '../ui';
import type { CheckBack } from '../core/generated/types.generated';
import { Badge } from '@/components/ui/badge';
import { Popover, PopoverContent, PopoverDescription, PopoverHeader, PopoverTitle, PopoverTrigger } from '@/components/ui/popover';

// When a chain's Chair checks back, as a chip in the chain's header; a click opens why, in the Chair's words, and while
// the chain waits (`waiting`) how to wake it sooner. It stands for the chain as long as the check-back does: also while
// the Chair's turn that planned it runs, and while the chain is paused. A chain that watches a signal (`checkBack.watch`, the
// one that ends first) gives its deadline instead, and the popover what it watches and the condition that wakes the Chair
// (ADR-0013); the chain's Overview and the Signals page list every watch of the chain.
// It is drawn as a status badge is (StandingBadge: its icon at 12px, its words), in the colour of a wait on time, a button
// that opens the popover.
defineProps<{ checkBack: CheckBack; waiting: boolean }>();
const same = (a: string, b: string) => Date.parse(a) === Date.parse(b);
</script>

<template>
  <Popover>
    <PopoverTrigger as-child>
      <Badge as="button" type="button" variant="gate" class="cursor-pointer" :data-deadline="checkBack.watch ? '' : undefined">
        <template v-if="checkBack.watch"><Icon name="clock" :size="12" /><span class="truncate">until {{ clockAt(checkBack.watch.until) }}</span></template>
        <template v-else><Icon name="clock" :size="12" /><span class="truncate">{{ clockAt(checkBack.at) }}</span></template>
      </Badge>
    </PopoverTrigger>
    <PopoverContent align="start" class="w-96">
      <PopoverHeader v-if="checkBack.watch">
        <PopoverTitle>The Chair watches <span class="mono [overflow-wrap:anywhere]" data-key>{{ checkBack.watch.key }}</span> until {{ clockAt(checkBack.watch.until) }}</PopoverTitle>
        <PopoverDescription class="whitespace-pre-line text-foreground">{{ checkBack.watch.why }}</PopoverDescription>
        <PopoverDescription>
          <template v-if="checkBack.watch.condition">It wakes the Chair when a reading meets <span class="mono [overflow-wrap:anywhere]" data-condition>{{ checkBack.watch.condition }}</span>, else at the deadline.</template>
          <template v-else>It wakes the Chair at every new reading, else at the deadline.</template>
        </PopoverDescription>
        <PopoverDescription v-if="!same(checkBack.at, checkBack.watch.until)">The Chair also checks back at {{ clockAt(checkBack.at) }}: {{ checkBack.why }}</PopoverDescription>
        <PopoverDescription v-if="waiting">Message the Chair to wake it now.</PopoverDescription>
      </PopoverHeader>
      <PopoverHeader v-else>
        <PopoverTitle>The Chair checks back at {{ clockAt(checkBack.at) }}</PopoverTitle>
        <PopoverDescription class="whitespace-pre-line text-foreground">{{ checkBack.why }}</PopoverDescription>
        <PopoverDescription v-if="waiting">Message the Chair to wake it now.</PopoverDescription>
      </PopoverHeader>
    </PopoverContent>
  </Popover>
</template>
