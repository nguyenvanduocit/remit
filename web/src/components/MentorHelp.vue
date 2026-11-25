<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { useAct } from '../act';
import { loadStream } from '../openChain';
import { clockAt } from '../ui';
import type { ChainView } from '../core/generated/types.generated';
import CardDrawer from './CardDrawer.vue';
import Callout from './Callout.vue';
import EmptyState from './EmptyState.vue';
import ReasonDialog from './ReasonDialog.vue';
import { Button } from '@/components/ui/button';

// The agent a mentor helps in the chain open beside their board, in the owner's drawer (CardDrawer): its stream, where it
// stands and the box to message it, which the server gives them once they accepted (ADR-0089). While the help is offered,
// what they are asked and by when, with Accept and Decline (asking why, which reaches the Chair) in the drawer's actions. Their newest help in the chain is the
// one shown.
const props = defineProps<{ view: ChainView; mentorId: string }>();
const act = useAct();
const help = computed(() =>
  props.view.helps.filter((h) => h.mentor === props.mentorId).sort((a, b) => b.offeredAt.localeCompare(a.offeredAt))[0],
);
const card = computed(() => props.view.cards.find((c) => c.id === help.value?.card));
const asked = computed(() => help.value?.state === 'offered');
// Once accepted the session is theirs to read: it is read again, as the offer could not read it.
watch(
  () => [card.value?.id, help.value?.state] as const,
  ([id, state], was) => id && state === 'accepted' && was?.[1] === 'offered' && loadStream(id),
);
const answering = ref(false);
const declining = ref(false);
async function answer(accept: boolean, why = '') {
  const h = help.value;
  if (!h || answering.value) return;
  answering.value = true;
  await (accept ? act('helpAccept', [h.id], 'You help this agent now') : act('helpDecline', [h.id, why], 'Declined'));
  answering.value = false;
  declining.value = false;
}
</script>

<template>
  <div class="flex min-h-0 min-w-0 flex-1 flex-col" data-mentor-help :data-state="help?.state">
    <Callout v-if="help && asked" class="m-3 mb-0" data-asked>
      <b>{{ view.chain.chain.title }}</b> asks your help: {{ help.problem }} <span class="text-mut">Answer by {{ clockAt(help.deadline) }}.</span>
    </Callout>
    <CardDrawer v-if="card" :view="view" :card="card" :read-only="help?.state !== 'accepted'">
      <template v-if="asked" #actions>
        <Button size="sm" :disabled="answering" data-accept @click="answer(true)">Accept</Button>
        <Button size="sm" variant="outline" :disabled="answering" data-decline @click="declining = true">Decline…</Button>
      </template>
    </CardDrawer>
    <EmptyState v-else class="p-6">You help no agent here.</EmptyState>
    <ReasonDialog
      v-model:open="declining"
      title="Decline this help?"
      :description="`${view.chain.chain.title}'s Chair is told, with your reason, and asks someone else.`"
      verb="Decline"
      busy="Declining"
      :running="answering"
      @confirm="(why) => answer(false, why)"
    />
  </div>
</template>
