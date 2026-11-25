<script setup lang="ts">
import { computed, ref } from 'vue';
import { useAct } from '../act';
import { plural } from '../ui';
import { Button } from '@/components/ui/button';
import Icon from './Icon.vue';
import ReasonDialog from './ReasonDialog.vue';
import Tip from './Tip.vue';

// Pausing a chain, wherever it is offered (its page, its line on the Subscriptions tab): the button asks first, saying what stops
// and that nothing is lost, and takes a why the chain's stream shows with who paused it (ADR-0080). `atWork` is how many of its
// agents the server says are at work, when the page holds them; unknown, the question says its agents.
const props = defineProps<{ chainId: string; atWork?: number; tip?: string }>();
const act = useAct();
const asking = ref(false);
const running = ref(false);
const stops = computed(() => {
  const n = props.atWork;
  if (n === undefined) return 'Its agents stop after their current step.';
  return n ? `${plural(n, 'agent')} ${n === 1 ? 'stops' : 'stop'} after the current step.` : 'Nothing starts until you resume.';
});
async function pause(why: string) {
  running.value = true;
  if (await act('chainPause', [props.chainId, why], 'Chain paused')) asking.value = false;
  running.value = false;
}
</script>

<template>
  <Tip :text="tip ?? 'Pause the chain: nothing starts until you resume it'">
    <Button variant="outline" size="sm" aria-label="Pause chain" @click="asking = true"><Icon name="pause" :size="12" /> Pause…</Button>
  </Tip>
  <ReasonDialog
    v-model:open="asking"
    title="Pause this chain?"
    :description="`${stops} Nothing is lost: a message you send waits, and the Chair reads it first on Resume.`"
    verb="Pause"
    busy="Pausing…"
    :running="running"
    @confirm="pause"
  />
</template>
