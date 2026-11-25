<script setup lang="ts">
import { computed, watch } from 'vue';
import { useAssistant } from '../nav';
import { agentOf, hasEarlier, loadEarlier, loadStream, opened, useCardEvents, waitingOn } from '../openChain';
import type { Card, ChainView } from '../core/generated/types.generated';
import { agentName } from '../ui';
import AgentDrawer from './AgentDrawer.vue';

// The agent picked in the open chain (?card=), in the drawer beside its page: its stream (its own newest page, asked for
// when the chain's answer held only the newest, and the pages before it as the owner asks), where it stands and what waits on the owner with it, its judgements, and the
// decisions: on the Chair the chain's, on any other agent those about its card (its hire, a hand-over, a person's answer to the
// offer). Its panel is the address's ?panel=. While the Assistant's column is open beside it the owner talks to the Assistant, so
// the drawer's own box to message the agent goes until the panel closes: one chat is live at a time, and none sits behind the other.
// A caller's buttons for the agent go in `actions` (a mentor's Accept and Decline).
// `readOnly`: no box to message it (a mentor whose help is not accepted).
const props = defineProps<{ view: ChainView; card: Card; readOnly?: boolean }>();
const { open: assistantOpen } = useAssistant();
const chair = computed(() => props.card.id === props.view.chain.chain.chairId);
const decisions = computed(() => props.view.decisions.map((d) => d.decision).filter((d) => chair.value || d.cardId === props.card.id));
// What this agent judged: of each judge, the rounds it gave. A reviewer gives all of its own; the goal checks are one judge
// whose rounds each come from a card of their own, so a check's card shows its round and not the newest.
const judged = computed(() =>
  props.view.verification.judges.flatMap((j) => {
    const given = [j.latest, ...j.earlier].filter((v) => v.cardId === props.card.id);
    return given.length ? [{ ...j, cardId: props.card.id, latest: given[0], earlier: given.slice(1) }] : [];
  }),
);
watch(() => props.card.id, (id) => loadStream(id), { immediate: true });
const events = useCardEvents(() => props.card.id);
</script>

<template>
  <AgentDrawer :card="card" :name="agentName(card)" :chair="chair" :agent="agentOf(card.id)" :events="events" :status="opened.status" :waiting="waitingOn(card.id)" :judged="judged" :decisions="decisions" panel-key="panel" :composer="!readOnly && !assistantOpen" :earlier="hasEarlier(card.id) ? () => loadEarlier(card.id) : undefined"><template v-if="$slots.actions" #actions><slot name="actions" /></template></AgentDrawer>
</template>
