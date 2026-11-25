<script setup lang="ts">
import { computed } from 'vue';
import { useAct } from '../act';
import { useStream } from '../streamContext';
import { receipt } from '../stream';
import { toneText } from '../ui';
import type { EventRead } from '../core/generated/types.generated';
import Tip from './Tip.vue';
import { Button } from '@/components/ui/button';

// The line under a message put to an agent: whether it has arrived, is being read, or was read, from the message's status
// and the message the server says the agent is on, in the colour of its tone. While it waits for the agent's current work,
// the owner may end that turn so it is read now, where the server lets him (a person's turn is theirs, so it offers no
// interrupt), or take his own message back: its words return to the agent's box, and the server refuses once the agent has
// read it or while that box holds other unsent words (ADR-0080).
const props = defineProps<{ event: EventRead }>();
const act = useAct();
const stream = useStream();
const agent = computed(() => stream.agentOf(props.event.event.cardId));
const r = computed(() => receipt(props.event, agent.value.onMessage));
const his = computed(() => props.event.event.by?.kind === 'owner');
</script>

<template>
  <div class="flex items-center gap-2 text-[10.5px]" data-receipt>
    <span :class="toneText[r.tone]">{{ r.text }}</span>
    <Tip v-if="r.queued && agent.asks.includes('interrupt')" text="Interrupt the running turn and read this message now"><Button variant="outline" size="xs" @click="act('cardCancel', [event.event.cardId], 'Turn interrupted — your message is read now')">Send now</Button></Tip>
    <Tip v-if="r.queued && his" text="Take this message back into the box, before the agent reads it"><Button variant="ghost" size="xs" data-withdraw @click="act('cardPromptWithdraw', [event.event.id], 'Message taken back into the box')">Take back</Button></Tip>
  </div>
</template>
