<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { useAct } from '../act';
import { assistant, assistantWaiting, loadAssistant } from '../assistant';
import { useAssistant, useWhere } from '../nav';
import { assistantStream, provideStream } from '../streamContext';
import type { AssistantView, Delivery } from '../core/generated/types.generated';
import type { Call } from '../core/client';
import AgentDrawer from './AgentDrawer.vue';
import AssistantFeed from './AssistantFeed.vue';
import AssistantRuntime from './AssistantRuntime.vue';
import Icon from './Icon.vue';
import Tip from './Tip.vue';
import { Button } from '@/components/ui/button';

// The Assistant, the owner's agent for the whole app, in its column beside whatever view is open (Shell): an agent's drawer
// over its card, its panel the address's ?assistantPanel=. What the owner sends carries where he is (useWhere), so "this one" means what he
// looks at. Its conversation is asked for when the panel first opens. Start over is offered when the server says it can be
// asked: the conversation shown begins anew at it (assistant.ts). The box takes the focus as it opens, so he can write at
// once, and Escape inside it closes it and gives the focus back to the top bar's button.
// Its Decisions panel is its feed: what it decided for the owner, and what he
// overturned (AssistantFeed), the tab counting what came since he last looked.
const props = defineProps<{ view: AssistantView }>();
const act = useAct();
const where = useWhere();
const { close } = useAssistant();
provideStream(assistantStream);
const deliver = (draftId: string, delivery: Delivery): Call => ({ method: 'assistant_prompt', args: { draftId, where: where.value, delivery } });

watch(
  () => props.view.assistant.cardId,
  () => {
    if (!assistant.loaded) loadAssistant();
  },
  { immediate: true },
);
const root = ref<HTMLElement | null>(null);
void nextTick(() => root.value?.querySelector<HTMLElement>('textarea, input')?.focus());
// However it closes (Esc, its X, the address), a focus that was in the panel, or already let go of (the composer blurs on Esc
// before the panel hears it), goes back to the top bar's button that opened it, so the keyboard does not restart at the
// page's top. A focus the owner put elsewhere stays there.
onBeforeUnmount(() => {
  const at = document.activeElement;
  if (!at || at === document.body || root.value?.contains(at)) document.querySelector<HTMLElement>('[data-assistant-toggle]')?.focus();
});
</script>

<template>
  <div ref="root" class="flex min-h-0 min-w-0 flex-1" data-assistant @keydown.esc="close()">
    <AgentDrawer :card="view.card" name="Assistant" :chair="false" :agent="view.agent" :events="assistant.events" :status="assistant.status" :waiting="assistantWaiting" :judged="[]" :decisions="[]" :decisions-new="assistant.feed?.unseen" panel-key="assistantPanel" :deliver="deliver">
      <template #title><span class="sr-only">Assistant</span></template>
      <template #composerOptions><AssistantRuntime :card="view.card" :agent="view.agent" /></template>
      <template #decisions><AssistantFeed /></template>
      <template #actions>
        <Tip v-if="view.agent.asks.includes('restart')" text="Clear this conversation and close its session: your next message opens a new one, which reads a recap of it">
          <Button variant="outline" size="sm" @click="act('cardRestart', [view.card.id], 'Started over')">Start over</Button>
        </Tip>
        <Tip text="Close (Esc). It keeps working.">
          <Button variant="ghost" size="icon-sm" aria-label="Close the Assistant" @click="close()"><Icon name="x" /></Button>
        </Tip>
      </template>
    </AgentDrawer>
  </div>
</template>
