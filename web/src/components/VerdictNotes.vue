<script setup lang="ts">
import Fold from './Fold.vue';
import Markdown from './Markdown.vue';
import StandingMark from './StandingMark.vue';
import { BLOCKER_SAYS, FINDING_LOOK, plural, type Look } from '../ui';
import type { Move, VerdictView } from '../core/generated/types.generated';
import { Button } from '@/components/ui/button';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible';

// What a judge wrote. Notes run long and open with the verdict, so its first line (as the server split it) is always in
// view and the proof behind it opens on request. Under it, what moved since the round before (the server compares them),
// then the findings, each folded over its evidence, which opens on a click; a screen reader says each one's status, which the eye reads from its icon.
defineProps<{ verdict: VerdictView }>();
const says = (s: keyof typeof FINDING_LOOK) => FINDING_LOOK[s].label;
// Where an item moved to, as a mark only: the words beside it say it, so a screen reader is not told twice.
const moveMark = (m: Move): Look => (m.to ? { tone: FINDING_LOOK[m.to].tone, icon: FINDING_LOOK[m.to].icon } : { tone: 'quiet', icon: 'dot' });
const moved = (m: Move) => (m.from && m.to ? `${says(m.from)} → ${says(m.to)}` : m.to ? `new, ${says(m.to)}` : `no longer listed, was ${says(m.from!)}`);
</script>

<template>
  <div class="flex flex-col gap-2">
    <Collapsible v-slot="{ open }">
      <Markdown type="body" :text="verdict.headline" />
      <CollapsibleContent><Markdown type="body" class="mt-2" :text="verdict.rest" /></CollapsibleContent>
      <CollapsibleTrigger v-if="verdict.rest" as-child>
        <Button variant="link" size="xs" class="mt-1 h-auto p-0">{{ open ? 'Show less' : `Show ${plural(verdict.more, 'more line')}` }}</Button>
      </CollapsibleTrigger>
    </Collapsible>
    <div v-if="verdict.moves.length" class="flex flex-col gap-1 text-xs" data-moves>
      <span class="font-medium text-mut">Since the round before</span>
      <ul class="flex flex-col gap-0.5">
        <li v-for="m in verdict.moves" :key="m.item" class="flex gap-1.5 leading-snug">
          <StandingMark v-bind="moveMark(m)" class="mt-0.5 self-start" />
          <span class="min-w-0 break-words">{{ m.item }}: <span class="text-mut">{{ moved(m) }}</span></span>
        </li>
      </ul>
    </div>
    <ul v-if="verdict.findings.length" class="mt-1 flex flex-col gap-2">
      <li v-for="(f, i) in verdict.findings" :key="i" class="flex gap-2 text-[13px] leading-snug">
        <StandingMark v-bind="FINDING_LOOK[f.status]" class="mt-0.5 self-start" />
        <Fold class="min-w-0 flex-1" data-finding>
          <template #summary>
            <span class="min-w-0 font-medium break-words">{{ f.item }}<span v-if="f.blocker" class="font-normal text-mut" data-blocker> · {{ BLOCKER_SAYS[f.blocker] }}</span></span>
          </template>
          <div class="mt-0.5 font-mono text-xs break-words whitespace-pre-wrap text-mut">{{ f.evidence }}</div>
        </Fold>
      </li>
    </ul>
  </div>
</template>
