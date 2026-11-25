<script setup lang="ts">
import { computed } from 'vue';
import { useRouter } from 'vue-router';
import { useAct } from '../act';
import { catalog } from '../catalog';
import { defaultTab } from '../nav';
import { clockAt } from '../ui';
import type { Impact, RetrospectiveRead } from '../core/generated/types.generated';
import Icon from './Icon.vue';
import InboxCallout from './InboxCallout.vue';
import Markdown from './Markdown.vue';
import StandingBadge from './StandingBadge.vue';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import type { BadgeVariants } from '@/components/ui/badge';

// What the coach read in the chain once it finished (ADR-0082): what went wrong, each with what it cost; the memories its
// sessions recalled, each marked helped or misled with why; the brief that would have served better, which starts a new chain
// as it is; and the lesson it proposed, when that raised a change that still waits on the owner, with its Apply and Not now.
// The retrospective, and the name of the coach that wrote it, as the server words it.
const props = defineProps<{ read: RetrospectiveRead }>();
const retrospective = computed(() => props.read.retrospective);
const act = useAct();
const router = useRouter();
const IMPACT: Record<Impact, { label: string; variant: BadgeVariants['variant']; icon: string }> = {
  high: { label: 'High', variant: 'danger', icon: 'alert' },
  medium: { label: 'Medium', variant: 'warning', icon: 'alert' },
  low: { label: 'Low', variant: 'quiet', icon: 'dot' },
};
const lesson = computed(() => (retrospective.value.lesson ? catalog.inbox.find((i) => i.id === retrospective.value.lesson) : undefined));
async function startAgain() {
  const made = await act('chainCreate', [{ intent: retrospective.value.brief }], 'Chain started from the better brief');
  if (made) void router.push({ name: 'chain', params: { id: made.id, tab: defaultTab } });
}
</script>

<template>
  <article class="retro flex flex-col gap-3 px-3.5 pt-2.5 pb-3 @container" aria-label="Retrospective" data-retro>
    <header class="flex min-w-0 items-center gap-2 text-xs">
      <Icon class="flex-none text-violet-text" name="spark" :size="13" />
      <b class="text-[12.5px] font-semibold text-text">Retrospective</b>
      <Badge variant="quiet" size="count" data-by>{{ read.byName }}</Badge>
      <time class="ml-auto flex-none text-[11px] text-mut tabular-nums" :datetime="retrospective.createdAt">{{ clockAt(retrospective.createdAt) }}</time>
    </header>
    <div class="grid gap-3 @[640px]:grid-cols-2">
      <section class="part" data-retro-issues>
        <h3 class="caps">What went wrong</h3>
        <ul v-if="retrospective.issues.length" class="grid grid-cols-[max-content_1fr] gap-x-2 gap-y-1.5">
          <li v-for="(issue, i) in retrospective.issues" :key="i" class="col-span-2 grid grid-cols-subgrid items-start">
            <StandingBadge :variant="IMPACT[issue.impact].variant" :icon="IMPACT[issue.impact].icon">{{ IMPACT[issue.impact].label }}</StandingBadge>
            <span class="min-w-0 text-text2 [overflow-wrap:anywhere]">{{ issue.text }}</span>
          </li>
        </ul>
        <p v-else class="text-mut">Nothing went wrong.</p>
      </section>
      <section class="part" data-retro-memories>
        <h3 class="caps">Memory that mattered</h3>
        <ul v-if="retrospective.memories.length" class="grid grid-cols-[max-content_1fr] gap-x-2 gap-y-1.5">
          <li v-for="m in retrospective.memories" :key="m.memoryId" class="col-span-2 grid grid-cols-subgrid items-start">
            <StandingBadge :variant="m.helped ? 'success' : 'danger'" :icon="m.helped ? 'check' : 'x'">{{ m.helped ? 'Helped' : 'Misled' }}</StandingBadge>
            <span class="min-w-0 [overflow-wrap:anywhere]"><q class="text-text2">{{ m.text }}</q><span v-if="m.why" class="text-mut"> · {{ m.why }}</span></span>
          </li>
        </ul>
        <p v-else class="text-mut">No memory was recalled.</p>
      </section>
    </div>
    <section class="part" data-retro-brief>
      <h3 class="caps">A better brief</h3>
      <Markdown type="quiet" :text="retrospective.brief" />
      <div class="mt-2"><Button variant="outline" size="sm" data-start-again @click="startAgain"><Icon name="play" :size="12" /> Start again with this brief</Button></div>
    </section>
    <InboxCallout v-if="lesson" :item="lesson" data-retro-lesson />
  </article>
</template>

<style scoped>
/* The coach's reading of a finished chain, in the feed as a card of its own, headed in its violet. */
.retro {
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--panel);
}
.part {
  padding: 9px 11px;
  border-radius: 8px;
  background: var(--panel2);
  font-size: 12.5px;
  line-height: 1.5;
}
.part h3 {
  margin-bottom: 6px;
}
</style>
