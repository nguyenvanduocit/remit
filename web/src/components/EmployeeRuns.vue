<script setup lang="ts">
import type { RouteLocationRaw } from 'vue-router';
import type { EmployeeRuns, Run } from '../core/generated/types.generated';
import type { BadgeVariants } from '@/components/ui/badge';
import { Badge } from '@/components/ui/badge';
import { fullTime, plural, span } from '../ui';
import Ago from './Ago.vue';
import Icon from './Icon.vue';
import ListState from './ListState.vue';
import Tip from './Tip.vue';

// An employee's runs across chains, as the server answers them (EmployeeRuns): newest first, each the chain it sat in, the
// first line of what it was asked, how it ended in the server's word (accepted, sent back, failed, stopped, open) as a badge
// with its icon, whether it waits, where the ask came from, when it started and how long it took (or that it runs); a run
// opens the agent in its chain. Over the whole list, the server's count of failures and of what waits; `more` says older
// runs are left out. Nothing is counted here.
// Reading, failing and empty read as every list the screen asks for does (ListState); `retry` is useRemote's.
defineProps<{ runs: EmployeeRuns | undefined; error: string; retry?: () => void }>();

// How each of the server's outcomes looks; one the list does not name reads as its word, quiet.
const LOOK: Record<string, { variant: BadgeVariants['variant']; icon: string }> = {
  accepted: { variant: 'success', icon: 'check' },
  'sent back': { variant: 'warning', icon: 'repeat' },
  failed: { variant: 'danger', icon: 'alert' },
  stopped: { variant: 'quiet', icon: 'stop' },
  open: { variant: 'info', icon: 'play' },
};
const look = (r: Run) => LOOK[r.outcome] ?? { variant: 'quiet', icon: 'circle' };
const SOURCE: Record<string, string> = { chair: 'hired by the Chair', agent: 'spawned by an agent', remit: 'a judge Remit seated' };
const source = (r: Run) => SOURCE[r.source] ?? `from ${r.source}`;
// How long a run took, from its start to the server's end; one the server says is at its work runs, and one that ended
// before ends were recorded shows its start alone.
const took = (r: Run): string | undefined =>
  r.live ? 'running' : r.ended ? `took ${span(new Date(r.ended).getTime() - new Date(r.started).getTime())}` : undefined;
const to = (r: Run): RouteLocationRaw => ({ name: 'chain', params: { id: r.chainId, tab: 'live' }, query: { card: r.cardId } });
</script>

<template>
  <ListState what="its runs" :held="!!runs" :error="error" :empty="!runs?.runs.length" :retry="retry">
    <template #empty><span data-runs-empty>No run yet: it has held no card.</span></template>
    <div v-if="runs" class="flex flex-col gap-2">
      <p class="m-0 text-xs text-mut" data-runs-summary>
        {{ plural(runs.failures, 'failure') }} · {{ runs.waiting }} waiting<template v-if="runs.more"> · the newest {{ runs.runs.length }} shown</template>
      </p>
      <ol class="m-0 flex list-none flex-col p-0" aria-label="Runs, newest first">
        <li v-for="r in runs.runs" :key="r.cardId" class="run" :data-run="r.cardId" :data-outcome="r.outcome">
          <RouterLink :to="to(r)" class="flex min-w-0 flex-col gap-0.5 rounded-md px-2 py-1.5 hover:bg-panel2">
            <span class="flex min-w-0 items-center gap-2">
              <Badge :variant="look(r).variant" class="flex-none" data-outcome-badge><Icon :name="look(r).icon" :size="11" />{{ r.outcome }}</Badge>
              <Badge v-if="r.waiting" variant="needs" class="flex-none" data-waiting><Icon name="clock" :size="11" />waiting</Badge>
              <Tip :text="r.chainTitle" when-clipped><span class="min-w-0 truncate text-[12.5px] font-medium text-text" data-chain>{{ r.chainTitle || r.chainId }}</span></Tip>
              <Tip :text="fullTime(r.started)"><time class="ml-auto flex-none text-[11.5px] text-mut tabular-nums" :datetime="r.started"><Ago :at="r.started" /></time></Tip>
              <span v-if="took(r)" class="flex-none text-[11.5px] text-mut tabular-nums" data-took>{{ took(r) }}</span>
            </span>
            <span class="flex min-w-0 items-center gap-2 text-[12px]">
              <span class="min-w-0 truncate text-text2" data-request>{{ r.request || 'no brief' }}</span>
              <span class="flex-none text-mut" data-source>{{ source(r) }}</span>
            </span>
          </RouterLink>
        </li>
      </ol>
    </div>
  </ListState>
</template>

<style scoped>
/* runs read as one list: a hairline between them, none after the last */
.run + .run {
  border-top: 1px dashed var(--rule);
}
.run > a {
  color: inherit;
  text-decoration: none;
}
</style>
