<script setup lang="ts">
import { useIntervalFn } from '@vueuse/core';
import { computed, ref, useTemplateRef, watchPostEffect } from 'vue';
import { attempt } from '../act';
import { useApi } from '../core/api';
import { dataOf, stepLook } from '../stream';
import { durSec } from '../ui';
import { useRemote } from '../useRemote';
import type { EventRead } from '../core/generated/types.generated';
import CodeBlock from './CodeBlock.vue';
import CallCard from './CallCard.vue';
import { Button } from '@/components/ui/button';

// One background task of an agent (a Monitor, a shell run in the background), drawn as a tool call is (CallCard): the
// tool, what it does and how it stands, and opened, what it runs and what it has written, read from the agent's process
// every few seconds while it runs and as the server kept it once it ended. The owner can stop one that runs; the agent
// goes on and is told. Both go to the Rust server (`readable` and `stoppable` are its words for what the task offers).
const props = defineProps<{ event: EventRead }>();
const api = useApi();

const open = ref(false);
// The tools that start a background task, by the icon a tool call of theirs shows.
const ICON: Record<string, string> = { Bash: 'terminal', Monitor: 'eye' };
const data = computed(() => dataOf(props.event));
const look = computed(() => stepLook(props.event.event.status));
const running = computed(() => look.value.working);
const ms = computed(() => Date.parse(props.event.event.updatedAt) - Date.parse(props.event.event.createdAt));

// While it runs and shows, its output is asked again every 2 s; ended, once.
const tick = ref(0);
useIntervalFn(() => open.value && running.value && tick.value++, 2000);
const output = useRemote(
  () => (open.value && data.value.readable ? { eventId: props.event.event.id, at: running.value ? tick.value : -1 } : undefined),
  ({ eventId }) => api.cardTaskOutput(eventId),
);
// A tail reads from its newest line: each new read scrolls the output to its end.
const outputBox = useTemplateRef<{ $el: HTMLElement }>('outputBox');
watchPostEffect(() => {
  const el = output.data.value?.text !== undefined ? outputBox.value?.$el : undefined;
  if (el) el.scrollTop = el.scrollHeight;
});
const stopping = ref(false);
async function stop() {
  stopping.value = true;
  await attempt(() => api.cardTaskStop(props.event.event.id), 'Background task stopped', 'Could not stop it');
  stopping.value = false;
}
</script>

<template>
  <CallCard v-model:open="open" class="my-1.5" :icon="ICON[data.tool ?? ''] ?? 'gear'" :name="data.tool || 'task'" :summary="event.event.title" :look="look" :time="running ? 'running…' : durSec(ms)" :failed="event.event.status === 'failed'" :data-work="event.event.id" data-task>
    <template #actions>
      <Button v-if="running && data.stoppable" variant="outline" size="xs" class="mr-2 flex-none" :disabled="stopping" data-task-stop @click="stop">Stop</Button>
    </template>
    <div class="flex flex-col gap-1.5" data-task-detail>
      <template v-if="data.command"><div class="caps">Command</div><CodeBlock class="max-h-[120px] overflow-auto" :text="data.command" label="Copy command" wrap /></template>
      <template v-if="data.readable">
        <div class="caps">Output<span v-if="output.data.value?.truncated" class="normal-case text-faint"> · last 8 KiB of {{ Math.round(output.data.value.totalBytes / 1024) }} KiB</span></div>
        <div v-if="output.error.value && !output.data.value" class="text-red-text" role="alert">Could not read its output: {{ output.error.value }}</div>
        <div v-else-if="!output.data.value" class="text-faint" role="status">Reading its output…</div>
        <div v-else-if="!output.data.value.text.trim()" class="text-faint">{{ running ? 'Nothing written yet.' : 'It wrote nothing.' }}</div>
        <CodeBlock v-else ref="outputBox" class="max-h-[260px] overflow-auto" :text="output.data.value.text.trim()" label="Copy output" wrap data-task-output />
      </template>
      <div v-else-if="!data.command" class="text-faint">Remit cannot read what this task writes.</div>
    </div>
  </CallCard>
</template>
