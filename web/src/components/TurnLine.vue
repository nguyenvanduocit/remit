<script setup lang="ts">
import { computed } from 'vue';
import type { EventRead, Touched, TurnSummary } from '../core/generated/types.generated';
import { durSec } from '../ui';
import Icon from './Icon.vue';
import StandingMark from './StandingMark.vue';
import Tip from './Tip.vue';

// Where a turn ended, as one row: how long it took and what it touched, each thing once with how many times, as the server
// summed it up (`turn`, TurnSummary). The turn's steps are folded into this row by the stream; opened, they unfold right
// under it (the default slot), where the owner clicked. What failed and what was refused stand under the row, never folded.
// A turn the server recorded no row for says how long it took.
const props = defineProps<{ event: EventRead; ms?: number; turn?: TurnSummary; steps: number }>();
const open = defineModel<boolean>('open', { default: false });

const ended = computed(() => props.event.event.status === 'end_turn');
const headline = computed(() => {
  if (props.ms === undefined) return ended.value ? 'Turn ended' : `Stopped (${props.event.event.status})`;
  return ended.value ? `Worked for ${durSec(props.ms)}` : `Stopped (${props.event.event.status}) after ${durSec(props.ms)}`;
});
const times = (t: Touched) => (t.count > 1 ? ` ×${t.count}` : '');
// A long turn's row tells what it touched first, not all of it.
const preview = computed(() => {
  const t = props.turn;
  if (!t) return '';
  const said = t.touched.map((x) => (x.summary || x.name) + times(x)).join(', ');
  return t.partial && said ? `${said}, …` : said;
});
</script>

<template>
  <div class="turn-line" data-turn-end>
    <button v-if="steps" type="button" class="row" :aria-expanded="open" @click="open = !open">
      <Icon :name="open ? 'chevron-down' : 'chevron-right'" :size="12" class="flex-none" />
      <span class="flex-none whitespace-nowrap">{{ headline }}</span>
      <Tip v-if="preview" :text="preview" when-clipped><span class="line min-w-0 flex-1 truncate text-faint">· {{ preview }}</span></Tip>
    </button>
    <div v-else class="row">
      <span class="flex-none whitespace-nowrap">— {{ headline }}</span>
      <Tip v-if="preview" :text="preview" when-clipped><span class="line min-w-0 flex-1 truncate text-faint">· {{ preview }}</span></Tip>
    </div>

    <div v-if="open && steps" class="steps" data-turn-steps><slot /></div>

    <ul v-if="turn?.failures.length" class="mt-1 flex flex-col gap-0.5" :aria-label="`What did not go through: ${turn.failures.length}`" data-turn-failures>
      <li v-for="f in turn.failures" :key="f.eventId" class="failure">
        <StandingMark tone="danger" :icon="f.kind === 'refused' ? 'x' : 'alert'" />
        <span class="flex-none font-medium text-red-text">{{ f.kind === 'refused' ? 'Refused' : 'Failed' }}</span>
        <span class="flex-none text-mut">{{ f.name }}</span>
        <Tip :text="f.summary" when-clipped><span class="line min-w-0 flex-1 truncate text-text2">{{ f.summary }}</span></Tip>
      </li>
    </ul>
  </div>
</template>

<style scoped>
/* The row: one line, 24px tall inside so it is a target the pointer finds (WCAG 2.5.8). */
.row {
  font-size: var(--size-meta);
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  min-width: 0;
  min-height: 24px;
  border-radius: 4px;
  color: var(--mut);
  text-align: left;
}
button.row {
  cursor: pointer;
}
button.row:hover {
  color: var(--text2);
}
/* A line cut to the width it is given: it lends none of its length to the stream's width (a run in the Live feed sizes to
   its content). */
.line {
  contain: inline-size;
}
/* The turn's steps, unfolded under its row and set in from it; they keep the stream's own sizes. */
.steps {
  margin: 2px 0 4px 18px;
}
.failure {
  font-size: var(--size-meta);
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  min-height: 20px;
}
</style>
