<script setup lang="ts">
import { computed, ref, useId } from 'vue';
import { useAct } from '../act';
import { useDraftBox } from '../drafts';
import { imageUrl, isInlineImage, putAttachment } from '../attachments';
import { messageOf, toast } from '../act';
import type { AgentStanding, Ask, Card, Delivery } from '../core/generated/types.generated';
import type { Call } from '../core/client';
import { Button } from '@/components/ui/button';
import { Textarea } from '@/components/ui/textarea';
import { onComposerKey } from '../composerKeys';
import { cardOf } from '../openChain';
import { agentName } from '../ui';
import ReasonDialog from './ReasonDialog.vue';
import { session } from '../session';
import Icon from './Icon.vue';
import Tip from './Tip.vue';
import Spinner from './Spinner.vue';

// The box to message the agent, with what the server says the owner can ask of it (`agent.asks`): a message to its open
// session, or one that resumes its closed session (and reopens a finished chain); with neither, a restart when it has no
// session left, or nothing at all (a judge that never opened a session). What is typed is the card's message draft, which the server
// keeps as it is typed: the same on every page, and there after a reload. Under the box, stopping the agent on the left (it asks first),
// then what Send does now in the server's words (`agent.onSend`: it waits for the turn in progress, the chain is paused) when the
// agent does not read it at once, and the sends on the right, wrapping at the drawer's narrowest: Send queues
// the message (read after the turn in progress, at once when none runs), and while a turn runs the server may offer Steer
// (into that turn) and Interrupt (the turn ends and the message is read next) as choices of their own (ADR-0080). `deliver`
// sends the draft: to a chain's agent (cardPrompt) unless given, the Assistant's panel gives its own (assistantPrompt). A card
// Claude Code spawned runs inside its parent's process and takes no message of its own: it says to talk to the parent, and
// keeps its Stop.
type Deliver = (draftId: string, delivery: Delivery) => Call;
const props = defineProps<{ card: Card; name: string; chair: boolean; agent: AgentStanding; deliver?: Deliver }>();
const emit = defineEmits<{ sent: [] }>();
const deliver: Deliver = (draftId, delivery) => (props.deliver ? props.deliver(draftId, delivery) : { method: 'card_prompt', args: { draftId, delivery } });
const act = useAct();
const can = (a: Ask) => props.agent.asks.includes(a);
const writes = computed(() => can('message') || can('resume'));
const parent = computed(() => {
  const id = props.card.spawn?.parent;
  const c = id ? cardOf(id) : undefined;
  return c ? agentName(c) : id ? 'the card that spawned it' : '';
});

let delivery: Delivery = 'queue'; // the choice of the send on its way: the box's send reads it
const message = useDraftBox(() => ({ home: props.card.home, cardId: props.card.id }), (draftId) => deliver(draftId, delivery));
const { text: draft, attachments } = message;
const unsent = computed(() => draft.value.trim() !== '' || attachments.value.length > 0);
const picker = ref<HTMLInputElement>();
const putting = ref(0);
async function attach(files: readonly File[]) {
  const home = props.card.home;
  const cardId = props.card.id;
  putting.value += files.length;
  for (const file of files) {
    try {
      const attachment = await putAttachment(file, home);
      if (props.card.id === cardId && props.card.home === home) {
        attachments.value = [...attachments.value, attachment];
      }
    } catch (e) {
      toast(`Could not attach ${file.name}: ${messageOf(e)}`);
    } finally {
      putting.value--;
    }
  }
}
function onPaste(e: ClipboardEvent) {
  const files = Array.from(e.clipboardData?.files ?? []);
  if (!files.length) return;
  e.preventDefault();
  void attach(files);
}
function onDrop(e: DragEvent) {
  const files = Array.from(e.dataTransfer?.files ?? []);
  if (files.length) void attach(files);
}
function onPick(e: Event) {
  const input = e.target as HTMLInputElement;
  void attach(Array.from(input.files ?? []));
  input.value = '';
}
const unattach = (id: string) => (attachments.value = attachments.value.filter((i) => i.id !== id));
// The box never holds: the words leave it at once for a pending row at the end of the stream, which says how they go
// (ADR-0094; AgentDrawer shows them, so the box never moves).
function sendAs(how: Delivery) {
  if (putting.value) return; // an attachment still on its way would be left out
  delivery = how;
  void message.send();
  emit('sent');
}
// With the link down a message still goes (when it is back); what acts on the agent now cannot, and says why.
const down = computed(() => session.blocked);
const saysId = useId();
// Stop shows when the server lets the owner stop the agent, and asks first, with what a stop does in the server's words
// (`agent.stopSays`, empty when he may not) and a why (the same question as a seat's Stop on the At work page).
const confirmStop = ref(false);
const stopping = ref(false);
async function stop(why: string) {
  stopping.value = true;
  if (await act('cardStop', [props.card.id, why], 'Agent stopped')) confirmStop.value = false;
  stopping.value = false;
}
</script>

<template>
  <div v-if="!writes" class="flex items-center justify-between gap-1.5 border-t border-line px-3 py-2.5">
    <span v-if="parent" class="text-xs text-mut" data-inside>{{ name }} runs inside {{ parent }}: talk to {{ parent }}.</span>
    <span v-else-if="can('restart')" class="text-xs text-mut">{{ name }} has no session to resume.</span>
    <span v-else class="text-xs text-mut">{{ name }} takes no more messages.</span>
    <Tip v-if="can('restart')" :text="down ?? `Start ${name} again`"><Button variant="outline" size="sm" class="aria-disabled:cursor-not-allowed aria-disabled:opacity-50" :aria-disabled="!!down" @click="down || act('cardRestart', [card.id], 'Restarting')">Restart</Button></Tip>
    <Tip v-if="agent.stopSays" :text="down ?? agent.stopSays"><Button variant="destructive" size="sm" class="aria-disabled:cursor-not-allowed aria-disabled:opacity-50" :aria-disabled="!!down" @click="down || (confirmStop = true)">Stop agent…</Button></Tip>
  </div>
  <div v-else class="border-t border-line px-3 pt-2.5 pb-2" data-composer>
    <div
      class="rounded-xl border border-line2 bg-panel2 shadow-[var(--shadow-card)] transition-[border-color,box-shadow] focus-within:border-blue-line focus-within:ring-3 focus-within:ring-blue-tint"
      @dragover.prevent
      @drop.prevent="onDrop"
    >
      <ul v-if="attachments.length || putting" class="flex flex-wrap gap-2 px-3 pt-2.5" aria-label="Attachments in this message" data-attachments>
        <li v-for="(i, n) in attachments" :key="i.id" class="relative min-w-0 max-w-full">
          <img v-if="isInlineImage(i.mediaType)" :src="imageUrl(i.id)" :alt="i.name || `Image ${n + 1}`" class="size-14 rounded-md border border-line object-cover" />
          <span v-else class="flex max-w-full items-center gap-1.5 rounded-md border border-line px-2 py-1.5 pr-4 text-xs" :title="i.name">
            <Icon name="paperclip" :size="13" class="shrink-0" /><span class="truncate">{{ i.name }}</span>
          </span>
          <button type="button" class="unattach" :aria-label="`Remove ${i.name || `image ${n + 1}`}`" @click="unattach(i.id)"><Icon name="x" :size="10" /></button>
        </li>
        <li v-if="putting" class="flex size-14 items-center justify-center rounded-md border border-dashed border-line2 text-mut" aria-live="polite"><Spinner :size="14" /><span class="sr-only">Uploading attachments…</span></li>
      </ul>
      <Textarea
        v-model="draft"
        rows="1"
        :aria-label="`Message ${name}`"
        :aria-describedby="agent.onSend ? saysId : undefined"
        :placeholder="can('message') ? (chair ? 'Talk to the Chair…' : `Message ${name}…`) : `Message to resume ${chair ? 'the Chair' : name}’s session…`"
        class="max-h-48 min-h-0 resize-none rounded-none border-0 bg-transparent px-3 pt-2.5 pb-1 text-[13px] leading-relaxed shadow-none outline-none focus-visible:outline-none focus-visible:ring-0 disabled:bg-transparent disabled:opacity-100"
        @keydown="onComposerKey($event, () => sendAs('queue'), () => ($event.target as HTMLElement).blur())"
        @paste="onPaste"
      />
      <div class="flex flex-wrap items-center gap-1.5 px-2 pt-1 pb-2">
        <input ref="picker" type="file" multiple class="hidden" @change="onPick" />
        <Tip text="Attach files (or paste or drop them in the box)">
          <Button variant="ghost" size="icon-sm" class="text-mut" aria-label="Attach files" data-attach @click="picker?.click()"><Icon name="paperclip" :size="14" /></Button>
        </Tip>
        <Tip v-if="agent.stopSays" :text="down ?? agent.stopSays">
          <Button variant="ghost" size="sm" class="text-mut hover:bg-red-tint hover:text-red-text aria-disabled:cursor-not-allowed aria-disabled:opacity-50 dark:hover:bg-red-tint" :aria-disabled="!!down" :data-down="!!down || undefined" @click="down || (confirmStop = true)"><Icon name="stop" :size="12" />Stop agent…</Button>
        </Tip>
        <!-- In the button row, which is always there, so the words coming and going with a turn never move the page. -->
        <span v-if="agent.onSend" :id="saysId" class="flex min-w-0 flex-1 items-center gap-1.5 px-1 text-[11.5px] text-mut" :title="agent.onSend" data-on-send><Icon class="flex-none" name="clock" :size="12" /><span class="truncate">{{ agent.onSend }}</span></span>
        <div class="ml-auto flex flex-wrap justify-end gap-1.5" data-sends>
          <Tip v-if="can('interrupt') && !unsent" :text="down ?? 'End the turn in progress. The agent stays available for your next message.'">
            <Button variant="outline" size="sm" class="rounded-full aria-disabled:cursor-not-allowed aria-disabled:opacity-50" :aria-disabled="!!down" :data-down="!!down || undefined" @click="down || act('cardCancel', [card.id], 'Turn interrupted')"><Icon name="interrupt" :size="13" />Interrupt</Button>
          </Tip>
          <template v-if="unsent">
            <Tip v-if="can('steer')" text="Put this into the turn in progress: the agent reads it at its next step"><Button variant="outline" size="sm" class="rounded-full" :disabled="!!putting" data-send="steer" @click="sendAs('steer')">Steer</Button></Tip>
            <Tip v-if="can('interrupt')" text="End the turn in progress: the agent reads this next"><Button variant="outline" size="sm" class="rounded-full" :disabled="!!putting" data-send="interrupt" @click="sendAs('interrupt')">Interrupt and send</Button></Tip>
          </template>
          <Button v-if="unsent || !can('interrupt')" size="sm" class="rounded-full pl-3" :disabled="!unsent || !!putting" data-send="queue" @click="sendAs('queue')">Send<Icon name="arrow-up" :size="13" /></Button>
        </div>
      </div>
    </div>
  </div>
  <ReasonDialog v-model:open="confirmStop" :title="`Stop ${name}?`" :description="agent.stopSays" verb="Stop agent" busy="Stopping…" :running="stopping" @confirm="stop" />
</template>

<style scoped>
.unattach {
  position: absolute;
  top: -6px;
  right: -6px;
  display: flex;
  width: 18px;
  height: 18px;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--line2);
  border-radius: 9999px;
  background: var(--panel);
  color: var(--mut);
}
.unattach:hover {
  color: var(--text);
}
.unattach:focus-visible {
  outline: 2px solid var(--blue-line);
  outline-offset: 1px;
}
</style>
