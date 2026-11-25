<script setup lang="ts">
import Icon from './Icon.vue';
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from '@/components/ui/input-group';

// A search over a list, wherever one is narrowed by what the owner types (the board's chains, an agent's context): the
// search icon in front, a clear button once something is typed, and Esc clears it. `label` names it for a screen reader; `size`
// is its step on the control ruler (ui/input), `sm` in a strip of page tabs.
defineProps<{ placeholder: string; label: string; size?: 'default' | 'sm' }>();
const query = defineModel<string>({ required: true });
</script>

<template>
  <InputGroup :size="size">
    <InputGroupAddon><Icon name="search" /></InputGroupAddon>
    <InputGroupInput v-model="query" :size="size" :placeholder="placeholder" :aria-label="label" autocomplete="off" @keydown.esc="query = ''" />
    <InputGroupAddon v-if="query" align="inline-end">
      <InputGroupButton size="icon-xs" aria-label="Clear search" @click="query = ''"><Icon name="x" /></InputGroupButton>
    </InputGroupAddon>
  </InputGroup>
</template>
