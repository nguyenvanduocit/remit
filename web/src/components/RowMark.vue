<script setup lang="ts">
import { ROW_BADGE } from '../tree';
import Icon from './Icon.vue';

// What something that is not an agent wears where an agent wears its badge, at the badge's size: an event signal, a
// chain. Its parent places it.
defineProps<{ icon: string }>();
</script>

<template>
  <span class="flex flex-none items-center justify-center rounded-[5px] bg-panel3 text-mut" :style="{ width: ROW_BADGE + 'px', height: ROW_BADGE + 'px' }" aria-hidden="true"><Icon :name="icon" :size="12" /></span>
</template>
