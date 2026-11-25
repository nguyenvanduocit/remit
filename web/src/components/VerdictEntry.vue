<script setup lang="ts">
import Ago from './Ago.vue';
import AgentBadge from './AgentBadge.vue';
import StandingBadge from './StandingBadge.vue';
import Tip from './Tip.vue';
import VerdictNotes from './VerdictNotes.vue';
import { CHECK_LOOK, fullTime, plural } from '../ui';
import type { Judge } from '../core/generated/types.generated';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible';

// One judge and what it found, as it stands now: its latest verdict, and its earlier rounds on request. Where `openCard` is
// given, each round links to the agent that gave it, which the owner opens to read its work or to ask it about its
// verdict; in that agent's own drawer there is nothing to open.
defineProps<{ judge: Judge; openCard?: (cardId: string) => void }>();
</script>

<template>
  <Card size="sm" :data-judge="judge.key">
    <CardHeader class="flex min-w-0 flex-row flex-wrap items-center gap-x-2 gap-y-1">
      <AgentBadge :agent="judge.role" :size="22" />
      <span class="truncate text-[13px] font-medium">{{ judge.name }}</span>
      <span v-if="judge.repo" class="font-mono text-xs text-mut">{{ judge.repo }}</span>
      <StandingBadge :variant="CHECK_LOOK[judge.latest.check.tone].tone" :working="judge.latest.check.tone === 'running'" :icon="CHECK_LOOK[judge.latest.check.tone].icon">{{ judge.latest.check.label }}</StandingBadge>
      <span class="ml-auto flex shrink-0 items-center gap-2 text-xs text-mut"
        ><Tip :text="fullTime(judge.latest.at)"><span><Ago :at="judge.latest.at" /></span></Tip
        ><Button v-if="openCard" variant="link" size="xs" class="h-auto p-0" @click="openCard(judge.latest.cardId)">Open agent</Button></span
      >
    </CardHeader>
    <CardContent class="flex flex-col gap-3">
      <VerdictNotes :verdict="judge.latest" />
      <Collapsible v-if="judge.earlier.length" v-slot="{ open }">
        <CollapsibleTrigger as-child>
          <Button variant="link" size="xs" class="h-auto p-0">{{ open ? 'Show less' : `Show ${plural(judge.earlier.length, 'earlier round')}` }}</Button>
        </CollapsibleTrigger>
        <!-- A display class on the content itself would override the hidden it carries while closed (and draw its border), so
             the layout sits inside it. -->
        <CollapsibleContent>
          <div class="mt-3 flex flex-col gap-4 border-t border-line pt-3">
            <div v-for="v in judge.earlier" :key="v.id" class="flex flex-col gap-2">
              <div class="flex items-center gap-2">
                <StandingBadge :variant="CHECK_LOOK[v.check.tone].tone" :working="v.check.tone === 'running'" :icon="CHECK_LOOK[v.check.tone].icon">{{ v.check.label }}</StandingBadge>
                <Tip :text="fullTime(v.at)"><span class="text-xs text-mut"><Ago :at="v.at" /></span></Tip>
                <Button v-if="openCard" variant="link" size="xs" class="ml-auto h-auto p-0" @click="openCard(v.cardId)">Open agent</Button>
              </div>
              <VerdictNotes :verdict="v" />
            </div>
          </div>
        </CollapsibleContent>
      </Collapsible>
    </CardContent>
  </Card>
</template>
