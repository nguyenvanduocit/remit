<script setup lang="ts">
import { computed, ref } from 'vue';
import { useRouter } from 'vue-router';
import Icon from './Icon.vue';
import { useSend } from '../act';
import { projectOf } from '../catalog';
import { onComposerKey } from '../composerKeys';
import { defaultTab, useWhere } from '../nav';
import type { Chain } from '../core/generated/types.generated';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Textarea } from '@/components/ui/textarea';

// A chain is asked for in words, as the owner would ask a person. It starts at once, and its Chair reads them and titles
// it; the directory is Remit's. It is a work chain (ADR-0113): a playbook it should follow is named in the words, and the
// other kinds start through the Assistant. Asked over a view of one project (the board or the
// Objectives tab narrowed to it, its page on Projects: Where.projectId), the chain is on that project (NewChain.project); else
// its Chair chooses the project too.
const emit = defineEmits<{ close: [] }>();
const router = useRouter();
const where = useWhere();
const projectId = computed(() => where.value.projectId);
const intent = ref('');
const { sending, send } = useSend(intent, (text) => ({ method: 'chain_create', args: { input: { intent: text, project: projectId.value } } }), 'Could not start the chain');
// The address opens it; what was typed stays when it did not go.
async function start() {
  // The server answers chain_create with the chain it made.
  const made = (await send()) as Chain | undefined;
  if (made) void router.push({ name: 'chain', params: { id: made.id, tab: defaultTab } });
}
// Esc belongs to the dialog, which closes on it: the box has nothing more to do.
const stay = () => {};
</script>

<template>
  <Dialog :open="true" @update:open="(open) => !open && emit('close')">
    <DialogContent class="sm:max-w-xl">
      <DialogHeader>
        <DialogTitle>New chain</DialogTitle>
        <DialogDescription>Say what you want done. It starts at once and its Chair titles it; the chain works on its own and lands its changes once its judges approve them.</DialogDescription>
      </DialogHeader>
      <form class="grid gap-2.5" @submit.prevent="start">
        <Textarea v-model="intent" autofocus class="max-h-[50vh] min-h-32 resize-y" aria-label="What you want done"
          placeholder="Example: Find the root cause of the empty body {} bug in the overview handler of customerfns, fix it, review it and verify."
          @keydown="onComposerKey($event, start, stay)" />
        <div class="flex min-h-7 flex-wrap items-center gap-2 text-xs text-mut">
          <template v-if="projectId">
            <span>On</span>
            <Badge variant="quiet" data-new-project><Icon name="project" :size="11" />{{ projectOf(projectId)?.name ?? projectId }}</Badge>
          </template>
          <span>Enter sends · Shift+Enter for a new line</span>
          <Button type="submit" size="sm" class="ml-auto" :disabled="sending || !intent.trim()">{{ sending ? 'Sending…' : 'Send' }}</Button>
        </div>
      </form>
    </DialogContent>
  </Dialog>
</template>
