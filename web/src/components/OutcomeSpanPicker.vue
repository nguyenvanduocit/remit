<script setup lang="ts">
import { computed } from 'vue';
import Icon from './Icon.vue';
import type { OutcomeRange, OutcomeSpan, OutcomeWindow } from '../core/generated/types.generated';
import { Button } from '@/components/ui/button';
import { DropdownMenu, DropdownMenuContent, DropdownMenuLabel, DropdownMenuRadioGroup, DropdownMenuRadioItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu';

// How far back the Outcomes page reads and the window it compares, as two menus of the server's choices (OutcomeSpan). A
// choice that does not hold the other twice is offered but disabled, so the owner sees why: the server would refuse it.
const props = defineProps<{ span: OutcomeSpan }>();
const last = defineModel<OutcomeRange>('last', { required: true });
const win = defineModel<OutcomeWindow>('window', { required: true });
const rangeLabel = computed(() => props.span.ranges.find((r) => r.key === props.span.range)?.label ?? props.span.range);
const windowLabel = computed(() => props.span.windows.find((w) => w.key === props.span.window)?.label ?? props.span.window);
// A menu answers with the value of the item picked: it is one of the server's choices.
const pickRange = (v: unknown) => {
  const r = props.span.ranges.find((x) => x.key === v);
  if (r) last.value = r.key;
};
const pickWindow = (v: unknown) => {
  const w = props.span.windows.find((x) => x.key === v);
  if (w) win.value = w.key;
};
</script>

<template>
  <div class="flex flex-none items-center gap-2" data-span>
    <DropdownMenu>
      <DropdownMenuTrigger as-child>
        <Button variant="outline" size="sm" :aria-label="`Time range: ${rangeLabel}`" data-range>
          <Icon name="clock" :size="13" /><span>{{ rangeLabel }}</span><Icon name="chevron-down" :size="12" class="text-mut" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" class="w-44">
        <DropdownMenuLabel class="text-xs text-mut">Time range</DropdownMenuLabel>
        <DropdownMenuRadioGroup :model-value="span.range" @update:model-value="pickRange">
          <DropdownMenuRadioItem v-for="r in span.ranges" :key="r.key" :value="r.key" :disabled="!r.fits" :data-range-choice="r.key">{{ r.label }}</DropdownMenuRadioItem>
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
    <DropdownMenu>
      <DropdownMenuTrigger as-child>
        <Button variant="outline" size="sm" :aria-label="`Time window: ${windowLabel}`" data-window>
          <Icon name="chart" :size="13" /><span>{{ windowLabel }}</span><Icon name="chevron-down" :size="12" class="text-mut" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" class="w-48">
        <DropdownMenuLabel class="text-xs text-mut">Compare each window with the one before</DropdownMenuLabel>
        <DropdownMenuRadioGroup :model-value="span.window" @update:model-value="pickWindow">
          <DropdownMenuRadioItem v-for="w in span.windows" :key="w.key" :value="w.key" :disabled="!w.fits" :data-window-choice="w.key">{{ w.label }}</DropdownMenuRadioItem>
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  </div>
</template>
