<script setup lang="ts">
import { Button } from '@/components/ui/button';
import Icon from './Icon.vue';

// The way back to a feed's newest line while the owner reads older ones (useStickToBottom): how much arrived since he moved
// up (`unseen`) and, when the feed knows, who wrote it (`who`). The parent places it over the feed.
defineProps<{ unseen: number; who?: string }>();
defineEmits<{ click: [] }>();
</script>

<template>
  <Button size="sm" data-to-latest @click="$emit('click')">
    <span class="sr-only">Go to the latest:</span>
    <template v-if="unseen">{{ unseen }} new<span v-if="who" class="max-w-[220px] truncate font-normal">· {{ who }}</span></template>
    <template v-else>Latest</template>
    <Icon name="chevron-down" :size="12" />
  </Button>
</template>
