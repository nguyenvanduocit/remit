<script setup lang="ts">
import { nextTick, ref, useId } from 'vue';
import { useSend } from '../act';
import type { Decision } from '../core/generated/types.generated';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

// Overturning one decision made for the owner (an answer the Assistant gave for him, a duel judge's pick): a button that opens
// one box for his word, sent as overturn; `ask` is what the box asks him (what he decides instead, unless said otherwise). Where
// the decision shows, the overturn shows once the server pushes it; what did not go stays in the box, and he is told why.
const props = withDefaults(defineProps<{ decision: Decision; ask?: string }>(), { ask: 'What do you decide instead?' });
const opened = ref(false);
const word = ref('');
const box = ref<InstanceType<typeof Input> | null>(null);
const id = useId();
const { sending, send } = useSend(word, (text) => ({ method: 'overturn', args: { decisionId: props.decision.id, word: text } }), 'Could not overturn');
async function open() {
  opened.value = true;
  await nextTick();
  (box.value?.$el as HTMLInputElement | undefined)?.focus();
}
async function submit() {
  if ((await send()) !== undefined) opened.value = false;
}
</script>

<template>
  <form v-if="opened" class="mt-2 flex flex-col gap-1.5" data-overturn-form @submit.prevent="submit" @keydown.esc.stop="opened = false">
    <label :for="id" class="text-[11.5px] text-text2">{{ ask }}</label>
    <div class="flex gap-1.5">
      <Input :id="id" ref="box" v-model="word" size="sm" class="flex-1" placeholder="In a few words" data-overturn-word />
      <Button type="submit" size="sm" :disabled="sending || !word.trim()" data-overturn-send>Overturn</Button>
      <Button type="button" variant="ghost" size="sm" @click="opened = false">Cancel</Button>
    </div>
  </form>
  <Button v-else variant="outline" size="xs" class="mt-1.5" :aria-label="`Overturn: ${decision.text}`" data-overturn-open @click="open">Overturn</Button>
</template>
