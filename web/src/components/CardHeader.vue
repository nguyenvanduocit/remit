<script setup lang="ts">
import { agentName, doingText } from '../ui';
import type { AgentStanding, Card } from '../core/generated/types.generated';
import AgentBadge from './AgentBadge.vue';
import AgentStandingBadge from './AgentStandingBadge.vue';
import ContextMeter from './ContextMeter.vue';
import HelpBadges from './HelpBadges.vue';
import LiveDot from './LiveDot.vue';
import PageHead from './PageHead.vue';
import Tip from './Tip.vue';
import { Badge } from '@/components/ui/badge';
import { TabsTrigger } from '@/components/ui/tabs';

// Who the agent is and what it is doing (`agent`: where the server says it stands), whether something waits on the owner
// from it (its mark asks to be taken there: `needs`), and the drawer's panels to pick from, with how full its context is at the
// end of the panels' row (the meter asks for what fills it: `context`). It is a PageHead, so a drawer beside a page, or beside
// another drawer, has its rows at the same heights as theirs. The place it is shown in may name the agent otherwise (`title`:
// the Assistant's panel leaves it to the icon) and adds its own buttons (`actions`: start over, close) at the end of the first
// row. The Decisions panel is offered when there are decisions to show (`decided`), counting the new ones when the caller says
// how many (`decisionsNew`). A card a mentor was asked to help, or helps, says so beside the meter: the mentor's name and
// where the help stands.
defineProps<{ card: Card; chair: boolean; agent: AgentStanding; decided: boolean; decisionsNew?: number }>();
defineEmits<{ context: []; needs: [] }>();
const tab = defineModel<string>('tab', { required: true });
</script>

<template>
  <PageHead :title="agentName(card)" :tab="tab" @update:tab="tab = $event">
    <template #title>
      <div class="flex min-w-0 items-center gap-2">
        <AgentBadge :agent="chair ? 'chair' : card.agent" :size="22" />
        <slot name="title"><Tip :text="agentName(card)" when-clipped><h2 class="min-w-0 truncate text-[14px] font-bold">{{ agentName(card) }}<template v-if="chair"> · orchestrator</template></h2></Tip></slot>
      </div>
    </template>
    <template #standing>
      <Tip v-if="agent.waiting.length" text="What it waits for is pinned under the stream">
        <button type="button" class="flex-none" :aria-label="`${agent.label}: answer it`" @click="$emit('needs')">
          <AgentStandingBadge :agent="agent" />
        </button>
      </Tip>
      <div class="presence" :class="doingText(agent.doingTone) || 'text-text2'" role="status"><LiveDot :live="agent.doingTone === 'busy'" /><Tip :text="agent.doing" when-clipped><span class="min-w-0 truncate">{{ agent.doing }}</span></Tip></div>
    </template>
    <template #end><slot name="actions" /></template>
    <template #tabs>
      <TabsTrigger v-if="decided" value="decisions" :aria-label="decisionsNew ? `Decisions, ${decisionsNew} new` : undefined">Decisions<Badge v-if="decisionsNew" variant="info" size="count" data-decisions-new>{{ decisionsNew }}</Badge></TabsTrigger>
      <TabsTrigger value="activity">Activity</TabsTrigger>
      <TabsTrigger value="plan">Plan</TabsTrigger>
      <TabsTrigger value="session">Session</TabsTrigger>
      <TabsTrigger v-if="agent.remembers" value="memory">Memory</TabsTrigger>
    </template>
    <template #aside>
      <HelpBadges :card="card.id" />
      <ContextMeter :used="card.ctxUsed" :size="card.ctxSize" @show="$emit('context')" />
    </template>
  </PageHead>
</template>

<style scoped>
/* What the agent is doing, in the colour its doing tone names (doingText), its dot in the same; the dot pings while it works. It
   takes what the row leaves and truncates in it. */
.presence {
  display: flex;
  flex: 1;
  align-items: center;
  gap: 6px;
  min-width: 0;
  font-size: 12px;
  white-space: nowrap;
}
</style>
