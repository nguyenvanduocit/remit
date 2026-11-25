<script setup lang="ts">
import Icon from './Icon.vue';
import { computed, ref, watch } from 'vue';
import { useAct } from '../act';
import { standingOf } from '../catalog';
import { opened } from '../openChain';
import type { Chain } from '../core/generated/types.generated';
import { Button } from '@/components/ui/button';
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu';
import ConfirmDialog from './ConfirmDialog.vue';
import PauseChain from './PauseChain.vue';
import ReasonDialog from './ReasonDialog.vue';
import Tip from './Tip.vue';

// What the owner can do to the chain as a whole, as the server takes it (ChainStanding.asks), each button saying what it
// does: pause, resume, finish, archive. Pausing (PauseChain), archiving and unarchiving ask first, saying what happens and taking a why in
// a sentence, which the chain's stream shows with who did it (ADR-0080, ADR-0081); delete waits in the More menu, as it cannot
// be undone, and asks first.
const props = defineProps<{ chain: Chain }>();
const asks = computed(() => new Set(standingOf(props.chain.id).asks));
const act = useAct();

type Asked = 'archive' | 'unarchive';
const asking = ref<Asked | null>(null);
const running = ref(false);
// The agents a pause stops: those the server says are at work in the open chain; unknown when another chain is open.
const atWork = computed(() => (opened.view?.chain.chain.id === props.chain.id ? opened.view.agents.filter((a) => a.tone === 'running').length : undefined));
const QUESTION: Record<Asked, { title: string; description: string; verb: string; busy: string }> = {
  archive: { title: 'Archive this chain?', description: 'It leaves the list and the board. Its work, agents and history stay, and you can unarchive it.', verb: 'Archive', busy: 'Archiving…' },
  unarchive: { title: 'Unarchive this chain?', description: 'It shows in the list and the board again.', verb: 'Unarchive', busy: 'Unarchiving…' },
};
// What was asked last stays the question while the dialog closes, so its words do not change under the closing animation.
const last = ref<Asked>('archive');
watch(asking, (a) => a && (last.value = a));
const question = computed(() => QUESTION[asking.value ?? last.value]);
const open = computed({ get: () => asking.value !== null, set: (o) => !o && (asking.value = null) });
async function confirm(why: string) {
  const id = props.chain.id;
  running.value = true;
  const done = asking.value === 'archive' ? await act('chainArchive', [id, why], 'Chain archived') : await act('chainUnarchive', [id, why], 'Chain unarchived');
  if (done) asking.value = null;
  running.value = false;
}

const confirmDelete = ref(false);
const deleting = ref(false);
async function remove(id: string) {
  deleting.value = true;
  if (await act('chainDelete', [id], 'Chain deleted')) confirmDelete.value = false;
  deleting.value = false;
}
</script>

<template>
  <PauseChain v-if="asks.has('pause')" :chain-id="chain.id" :at-work="atWork" />
  <Button v-if="asks.has('resume')" variant="outline" size="sm" @click="act('chainResume', [chain.id, ''], 'Chain resumed')"><Icon name="play" :size="12" /> Resume</Button>
  <Tip v-if="asks.has('finish')" text="End the chain now, with what it has"><Button variant="outline" size="sm" @click="act('chainFinish', [chain.id, ''], 'Chain finished')"><Icon name="stop" :size="12" /> Finish</Button></Tip>
  <Tip v-if="asks.has('archive')" text="Hide it from the list and the board"><Button variant="outline" size="sm" @click="asking = 'archive'"><Icon name="archive" :size="12" /> Archive…</Button></Tip>
  <Tip v-if="asks.has('unarchive')" text="Show it in the list and the board again"><Button variant="outline" size="sm" @click="asking = 'unarchive'"><Icon name="archive" :size="12" /> Unarchive…</Button></Tip>
  <DropdownMenu v-if="asks.has('delete')">
    <DropdownMenuTrigger as-child><Button variant="outline" size="icon-sm" aria-label="More actions"><Icon name="more" :size="14" /></Button></DropdownMenuTrigger>
    <DropdownMenuContent align="end" class="w-44">
      <DropdownMenuItem variant="destructive" @select="confirmDelete = true"><Icon name="trash" :size="13" /> Delete chain…</DropdownMenuItem>
    </DropdownMenuContent>
  </DropdownMenu>
  <ReasonDialog v-model:open="open" v-bind="question" :running="running" @confirm="confirm" />
  <ConfirmDialog
    v-model:open="confirmDelete"
    title="Delete this chain permanently?"
    :description="`The agents, conversations and worktrees of “${chain.title}” will be lost and cannot be recovered. What it landed stays on its base. To just hide it, archive it instead.`"
    irreversible
    verb="Delete chain"
    busy="Deleting…"
    :running="deleting"
    @confirm="remove(chain.id)"
  />
</template>
