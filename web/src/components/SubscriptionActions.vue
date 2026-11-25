<script setup lang="ts">
import { useAct } from '../act';
import type { SubscriptionLine } from '../core/generated/types.generated';
import EventSignalRail from './EventSignalRail.vue';
import Icon from './Icon.vue';
import PauseChain from './PauseChain.vue';
import Tip from './Tip.vue';
import { Button } from '@/components/ui/button';

// The owner's rails on one subscription: wake a Chair now, pause or resume its chain, a routine or an event signal (a paused
// one takes no event from any writer), run a routine now; pausing a chain asks first (PauseChain). Each is the action the server takes for him; none moves a time or edits
// what is checked (that is telling the chain's Chair or the Assistant). A button's name starts with the words it shows, so the owner can say them to reach it.
defineProps<{ line: SubscriptionLine }>();
const act = useAct();
</script>

<template>
  <div class="flex flex-none items-center gap-1.5">
    <template v-if="(line.kind === 'check_back' || line.kind === 'watch') && line.chainId">
      <Tip v-if="line.state === 'paused'" text="Resume the chain: it checks back at its time again"><Button variant="outline" size="sm" aria-label="Resume chain" @click="act('chainResume', [line.chainId, ''], 'Chain resumed')"><Icon name="play" :size="12" /> Resume</Button></Tip>
      <template v-else>
        <Tip text="Wake its Chair now instead of at the planned time"><Button variant="outline" size="sm" aria-label="Check now: wake its Chair" @click="act('chainCheckBackNow', [line.chainId], 'Chair woken')"><Icon name="play" :size="12" /> Check now</Button></Tip>
        <PauseChain :chain-id="line.chainId" tip="Pause the chain: it keeps its time" />
      </template>
    </template>
    <template v-else-if="line.kind === 'routine'">
      <Tip v-if="line.state === 'paused'" text="Resume the routine: it starts its chains at its times again"><Button variant="outline" size="sm" aria-label="Resume routine" @click="act('routineResume', [line.id], 'Routine resumed')"><Icon name="play" :size="12" /> Resume</Button></Tip>
      <template v-else-if="line.state !== 'underway'">
        <Tip v-if="!line.chainId" text="Start its chain now instead of at its time"><Button variant="outline" size="sm" aria-label="Run now: start its chain" @click="act('routineRun', [line.id], 'Routine started')"><Icon name="play" :size="12" /> Run now</Button></Tip>
        <Tip v-if="line.state !== 'over'" text="Pause the routine: it starts no chain until you resume it"><Button variant="outline" size="sm" aria-label="Pause routine" @click="act('routinePause', [line.id], 'Routine paused')"><Icon name="pause" :size="12" /> Pause</Button></Tip>
      </template>
    </template>
  </div>
</template>
