<script setup lang="ts">
import { computed, nextTick, ref } from 'vue';
import { attempt } from '../act';
import { onComposerKey } from '../composerKeys';
import { useApi } from '../core/api';
import type { EventRead, TurnMark } from '../core/generated/types.generated';
import { useStream } from '../streamContext';
import Icon from './Icon.vue';
import Tip from './Tip.vue';
import { Button } from '@/components/ui/button';
import { Textarea } from '@/components/ui/textarea';

// Teaching an agent from one of its replies (ADR-0093): Helpful, Not helpful (with why, which may stay empty) or Correct
// reply (with the reply it should have given), each the owner's word to that agent, recorded as a teaching its memory
// learns from (`turn_teach`). The marks sit in a bar over the reply's top right, with whatever the parent puts beside
// them (`actions`); the parent is the positioned box the bar sits on, and shows the bar on hover and focus. The words go
// in a box under the reply, as a message does. His mark is the one the server keeps on the reply (its `marked` activity),
// so a reload or a second tab shows it, and his new mark replaces it.
const props = defineProps<{ event: EventRead }>();
const api = useApi();
const stream = useStream();
const agent = computed(() => stream.nameOf(props.event.event.cardId));

const marked = computed<TurnMark | null>(() => {
  const a = props.event.activity;
  return a?.type === 'marked' ? (a.marks.find((m) => m.by.kind === 'owner')?.mark ?? null) : null;
});
const writing = ref<Exclude<TurnMark, 'helpful'> | null>(null);
const words = ref('');
const sending = ref(false);
const box = ref<InstanceType<typeof Textarea> | null>(null);
const opener = ref<HTMLElement | null>(null);

const DONE: Record<TurnMark, string> = { helpful: 'Marked helpful', not_helpful: 'Marked not helpful', correct: 'Correct reply taught' };
async function teach(mark: TurnMark, text: string) {
  if (sending.value) return;
  sending.value = true;
  const ok = await attempt(() => api.turnTeach(props.event.event.id, mark, text), undefined, 'Could not teach it');
  sending.value = false;
  if (ok === undefined) return;
  close();
}
async function write(mark: Exclude<TurnMark, 'helpful'>, e: MouseEvent) {
  opener.value = e.currentTarget as HTMLElement;
  writing.value = writing.value === mark ? null : mark;
  await nextTick();
  (box.value?.$el as HTMLTextAreaElement | undefined)?.focus();
}
function close() {
  writing.value = null;
  words.value = '';
  opener.value?.focus();
}
const send = () => {
  if (writing.value && (writing.value !== 'correct' || words.value.trim())) void teach(writing.value, words.value.trim());
};
</script>

<template>
  <div class="teach">
    <div class="teach-bar" role="toolbar" :aria-label="`Teach ${agent} from this reply`">
      <Tip text="Helpful"><Button variant="ghost" size="icon-xs" aria-label="Helpful" :aria-pressed="marked === 'helpful'" :disabled="sending" data-teach="helpful" @click="teach('helpful', '')"><Icon name="check" :size="12" /></Button></Tip>
      <Tip text="Not helpful"><Button variant="ghost" size="icon-xs" aria-label="Not helpful" :aria-pressed="marked === 'not_helpful' || writing === 'not_helpful'" data-teach="not_helpful" @click="write('not_helpful', $event)"><Icon name="x" :size="12" /></Button></Tip>
      <Tip text="Correct reply"><Button variant="ghost" size="icon-xs" aria-label="Correct reply" :aria-pressed="marked === 'correct' || writing === 'correct'" data-teach="correct" @click="write('correct', $event)"><Icon name="pencil" :size="12" /></Button></Tip>
      <slot name="actions" />
    </div>
    <p v-if="marked && !writing" class="said" role="status"><Icon name="check" :size="11" />{{ DONE[marked] }}: {{ agent }} learns from it</p>
    <div v-if="writing" class="mt-2 flex flex-col gap-1.5" data-teach-box>
      <label :for="`teach-${event.event.id}`" class="text-meta text-mut">{{ writing === 'correct' ? `The reply ${agent} should have given` : `Why it did not help (you may leave it empty)` }}</label>
      <Textarea :id="`teach-${event.event.id}`" ref="box" v-model="words" class="min-h-14 text-read" @keydown="onComposerKey($event, send, close)" />
      <div class="flex items-center gap-1.5">
        <Button size="sm" :disabled="sending || (writing === 'correct' && !words.trim())" @click="send">{{ writing === 'correct' ? 'Teach this reply' : 'Mark not helpful' }}</Button>
        <Button variant="ghost" size="sm" @click="close">Cancel</Button>
        <span class="ml-auto text-meta text-faint">Enter sends · Esc cancels</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* The bar floats over the reply's top right, on the surface above it, and shows only while the reply is under the pointer
   or the bar holds the focus (the parent says when), so it never stays over the words: a mark is said under the reply,
   and an open box has its own buttons. */
.teach-bar {
  position: absolute;
  top: -14px;
  right: 0;
  display: flex;
  gap: 2px;
  padding: 2px;
  border-radius: 6px;
  background: var(--panel3);
  box-shadow: var(--shadow-card);
  opacity: 0;
}
@media (hover: none) {
  .teach-bar {
    opacity: 1;
  }
}
.said {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-top: 4px;
  color: var(--mut);
  font-size: var(--size-meta);
}
</style>
