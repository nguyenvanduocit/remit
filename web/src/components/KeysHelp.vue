<script setup lang="ts">
import { KEYS } from '../appKeys';
import Kbd from './Kbd.vue';
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog';

// The app's keys, listed (?): each key as it is pressed and what it does, on every page.
const open = defineModel<boolean>('open', { required: true });
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="sm:max-w-md" data-keys-help>
      <DialogHeader>
        <DialogTitle>Keys</DialogTitle>
        <DialogDescription>On every page, when you are not typing in a box.</DialogDescription>
      </DialogHeader>
      <dl class="m-0 grid grid-cols-[auto_1fr] items-center gap-x-4 gap-y-2.5 text-[12.5px]">
        <template v-for="k in KEYS" :key="k.says">
          <dt class="flex gap-1"><Kbd v-for="key in k.keys" :key="key">{{ key }}</Kbd></dt>
          <dd class="m-0 text-text2">{{ k.says }}</dd>
        </template>
      </dl>
    </DialogContent>
  </Dialog>
</template>
