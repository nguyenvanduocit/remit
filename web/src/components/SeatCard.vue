<script setup lang="ts">
import { computed, ref } from 'vue';
import { useRoute, type RouteLocationRaw } from 'vue-router';
import type { Seat } from '../core/generated/types.generated';
import { useAct } from '../act';
import { catalog, listedChain } from '../catalog';
import { plural } from '../ui';
import AgentStandingBadge from './AgentStandingBadge.vue';
import EmployeeFace from './EmployeeFace.vue';
import Icon from './Icon.vue';
import ItemCard from './ItemCard.vue';
import ReasonDialog from './ReasonDialog.vue';
import Tip from './Tip.vue';
import WorkLine from './WorkLine.vue';
import { Button } from '@/components/ui/button';

// One seat on the At work page, the card every list's items wear (ItemCard): the agent's name, its employee's face (the
// Assistant's mark, which is no employee), its badge, and under it what it does now (WorkLine), the chain it sits in as a
// chip to that chain, how many helpers work under it, and Stop when the server lets the owner stop it. Stop asks first, with
// what a stop does in the server's words (`stopSays`, empty when he may not), and takes a why (ADR-0080). A click on the
// card opens the agent where it works: its drawer on its chain's Live tab, or the Assistant's panel (`kind`).
const props = defineProps<{ seat: Seat }>();
const route = useRoute();
const act = useAct();

const employee = computed(() => (props.seat.employee ? catalog.employees.find((e) => e.employee.id === props.seat.employee) : undefined));
const assistant = computed(() => props.seat.kind === 'assistant');
const to = computed<RouteLocationRaw>(() => {
  if (assistant.value) return { query: { ...route.query, assistant: '1' } };
  const chair = listedChain(props.seat.home)?.chairId;
  return { name: 'chain', params: { id: props.seat.home, tab: 'live' }, query: { card: props.seat.cardId === chair ? undefined : props.seat.cardId } };
});
const chain = computed<RouteLocationRaw>(() => ({ name: 'chain', params: { id: props.seat.home, tab: 'overview' } }));

const asking = ref(false);
const stopping = ref(false);
async function stop(why: string) {
  stopping.value = true;
  if (await act('cardStop', [props.seat.cardId, why], `${props.seat.name} stopped`)) asking.value = false;
  stopping.value = false;
}
</script>

<template>
  <ItemCard :to="to" :on="false" :title="seat.name" :data-seat="seat.cardId" :data-tone="seat.tone">
    <template #chip>
      <EmployeeFace v-if="employee" :face="employee.face" :size="18" />
      <Icon v-else-if="assistant" name="assistant" :size="14" class="text-violet-text" />
    </template>
    <template #badge><AgentStandingBadge :agent="seat" lined /></template>
    <template #meta>
      <span v-if="employee" class="min-w-0 truncate" data-employee>{{ employee.shown }}</span>
    </template>
    <template #below>
      <div class="mt-1.5 flex flex-col gap-1.5">
        <WorkLine class="text-[12.5px]" lead="Now" wrap :work="seat.work" :since="seat.since" :warms="seat.warms" :doing="seat.doing" :doing-tone="seat.doingTone" />
        <div class="flex min-w-0 flex-wrap items-center gap-2 text-[11.5px]">
          <Tip v-if="!assistant" :text="seat.chainTitle" when-clipped>
            <RouterLink :to="chain" class="chip inline-flex max-w-[360px] min-w-0 items-center gap-1 rounded-md border border-line px-1.5 text-text2 hover:border-line2 hover:text-text" data-chain-chip>
              <Icon name="chain" :size="11" /><span class="truncate">{{ seat.chainTitle }}</span>
            </RouterLink>
          </Tip>
          <span v-if="seat.helpers.length" class="text-mut" data-helpers>{{ plural(seat.helpers.length, 'helper') }} at work</span>
          <Button v-if="seat.stopSays" variant="ghost" size="xs" class="ml-auto text-mut hover:bg-red-tint hover:text-red-text" :aria-label="`Stop ${seat.name}`" data-stop @click="asking = true"><Icon name="stop" :size="11" />Stop…</Button>
        </div>
      </div>
    </template>
  </ItemCard>
  <ReasonDialog v-model:open="asking" :title="`Stop ${seat.name}?`" :description="seat.stopSays" verb="Stop agent" busy="Stopping…" :running="stopping" @confirm="stop" />
</template>

<style scoped>
.chip {
  height: 20px;
  text-decoration: none;
}
</style>
