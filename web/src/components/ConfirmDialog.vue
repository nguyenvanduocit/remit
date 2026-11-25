<script setup lang="ts">
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import HoldButton from './HoldButton.vue';

// The question before a hard action (stop an agent, delete a chain): what it is (`title`), what happens (`description`),
// Cancel, and the action in its destructive colour, named by `verb` and by `busy` while it runs. One that cannot be undone at
// all (`irreversible`: a chain deleted, a run stopped for good) is confirmed by holding its button (HoldButton), not a click.
defineProps<{ title: string; description: string; verb: string; busy: string; running: boolean; irreversible?: boolean }>();
const open = defineModel<boolean>('open', { required: true });
const emit = defineEmits<{ confirm: [] }>();
</script>

<template>
  <Dialog :open="open" @update:open="open = $event">
    <DialogContent class="sm:max-w-lg">
      <DialogHeader>
        <DialogTitle>{{ title }}</DialogTitle>
        <DialogDescription>{{ description }}</DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button variant="outline" @click="open = false">Cancel</Button>
        <HoldButton v-if="irreversible" :label="verb" :busy="running ? busy : undefined" :disabled="running" @confirm="emit('confirm')" />
        <Button v-else variant="destructive" :disabled="running" @click="emit('confirm')">{{ running ? busy : verb }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
