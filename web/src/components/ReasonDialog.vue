<script setup lang="ts">
import { ref, watch } from 'vue';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';

// The question before an action whose why the server keeps (pausing, archiving or unarchiving a chain, declining a help):
// what it is (`title`), what happens (`description`), one line for why (empty is allowed), Cancel, and the action named by
// `verb` and by `busy` while it runs. Enter in the line confirms.
defineProps<{ title: string; description: string; verb: string; busy: string; running: boolean; destructive?: boolean }>();
const open = defineModel<boolean>('open', { required: true });
const emit = defineEmits<{ confirm: [why: string] }>();
const why = ref('');
watch(open, (o) => o && (why.value = ''));
const confirm = () => emit('confirm', why.value.trim());
</script>

<template>
  <Dialog :open="open" @update:open="open = $event">
    <DialogContent class="sm:max-w-lg">
      <DialogHeader>
        <DialogTitle>{{ title }}</DialogTitle>
        <DialogDescription>{{ description }}</DialogDescription>
      </DialogHeader>
      <Input v-model="why" autocomplete="off" aria-label="Why, in a sentence (optional)" placeholder="Why, in a sentence (optional)" data-why @keydown.enter.exact.prevent="!running && confirm()" />
      <DialogFooter>
        <Button variant="outline" @click="open = false">Cancel</Button>
        <Button :variant="destructive ? 'destructive' : 'default'" :class="destructive && 'text-red-text focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-red-text'" :disabled="running" data-confirm @click="confirm">{{ running ? busy : verb }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
