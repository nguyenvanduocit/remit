<script setup lang="ts">
import { computed, useId } from 'vue';
import type { BlockPart } from '../core/generated/types.generated';
import Icon from './Icon.vue';
import { Checkbox } from '@/components/ui/checkbox';
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group';

// One choice of a block (ADR-0091): its options as cards, each with the line that compares it, one to pick or several when
// `many`. `picked` is what it shows and builds the next pick from: the newest pick still on its way to the server, else the
// server's (`part.picked`), so quick clicks each add to the last rather than to an answer not back yet. A pick shows by a
// check and the word "Picked", never by colour alone; a change is emitted as the whole new pick for the block to send.
// Labels and details are plain text, never Markdown.
type Choice = Extract<BlockPart, { type: 'choice' }>;
const props = defineProps<{ part: Choice; picked: string[] }>();
const emit = defineEmits<{ pick: [picked: string[]] }>();
const id = useId();

const on = computed(() => new Set(props.picked));
const one = computed(() => props.picked[0] ?? '');
function toggle(option: string, tick: boolean) {
  const next = props.part.options.map((o) => o.id).filter((o) => (o === option ? tick : on.value.has(o)));
  emit('pick', next);
}
</script>

<template>
  <fieldset class="min-w-0" :data-choice="part.id">
    <legend class="caps mb-1.5">{{ part.label }}<span v-if="part.many" class="normal-case text-faint"> · pick any</span></legend>
    <RadioGroup v-if="!part.many" :model-value="one" class="gap-1.5" :aria-label="part.label" @update:model-value="(v) => typeof v === 'string' && v !== one && emit('pick', [v])">
      <label v-for="o in part.options" :key="o.id" :for="`${id}-${o.id}`" class="option" :class="{ on: on.has(o.id) }" :data-option="o.id">
        <RadioGroupItem :id="`${id}-${o.id}`" :value="o.id" class="mt-[3px]" />
        <span class="min-w-0 flex-1">
          <span class="name">{{ o.label }}</span>
          <span v-if="o.detail" class="detail">{{ o.detail }}</span>
        </span>
        <span v-if="on.has(o.id)" class="mark"><Icon name="check" :size="12" />Picked</span>
      </label>
    </RadioGroup>
    <div v-else class="flex flex-col gap-1.5">
      <label v-for="o in part.options" :key="o.id" :for="`${id}-${o.id}`" class="option" :class="{ on: on.has(o.id) }" :data-option="o.id">
        <Checkbox :id="`${id}-${o.id}`" :model-value="on.has(o.id)" class="mt-[3px]" @update:model-value="(v) => toggle(o.id, v === true)" />
        <span class="min-w-0 flex-1">
          <span class="name">{{ o.label }}</span>
          <span v-if="o.detail" class="detail">{{ o.detail }}</span>
        </span>
        <span v-if="on.has(o.id)" class="mark"><Icon name="check" :size="12" />Picked</span>
      </label>
    </div>
  </fieldset>
</template>

<style scoped>
/* An option: a card the whole of which picks it; the picked one is pressed into the page (a surface, never a ring). */
.option {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 8px 10px;
  border-radius: 8px;
  background: var(--panel2);
  cursor: pointer;
}
.option:hover {
  background: var(--panel3);
}
.option.on {
  background: var(--well);
  box-shadow: var(--shadow-pressed);
}
.name {
  display: block;
  color: var(--text);
  font-size: var(--size-read);
  font-weight: 500;
  line-height: var(--leading-read);
  overflow-wrap: anywhere;
}
.detail {
  display: block;
  color: var(--text2);
  font-size: var(--size-read);
  line-height: var(--leading-read);
  overflow-wrap: anywhere;
}
.mark {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: 4px;
  margin-top: 3px;
  color: var(--text);
  font-size: var(--size-meta);
  font-weight: 500;
}
</style>
