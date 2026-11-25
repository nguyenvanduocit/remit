<script setup lang="ts">
import { Button } from '@/components/ui/button';
import Icon from './Icon.vue';

// The arrows at the end of a column's name, beside an open item: they slide to the column before and after it, and the one that
// leads nowhere is off.
defineProps<{ prev?: { key: string; label: string }; next?: { key: string; label: string } }>();
defineEmits<{ go: [key: string] }>();
</script>

<template>
  <div class="flex flex-none items-center normal-case">
    <Button variant="ghost" size="icon-xs" :disabled="!prev" :aria-label="prev ? `Previous column: ${prev.label}` : 'No previous column'" @click="prev && $emit('go', prev.key)"><Icon name="chevron-left" /></Button>
    <Button variant="ghost" size="icon-xs" :disabled="!next" :aria-label="next ? `Next column: ${next.label}` : 'No next column'" @click="next && $emit('go', next.key)"><Icon name="chevron-right" /></Button>
  </div>
</template>
