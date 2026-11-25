<script setup lang="ts">
import { computed } from 'vue';
import StandingMark from './StandingMark.vue';
import Tip from './Tip.vue';
import { OUTCOME_LOOK, plural, span as duration, toneText } from '../ui';
import type { OutcomeKind, OutcomeKpi, OutcomePoint, OutcomeReads, OutcomeSpan } from '../core/generated/types.generated';
import { Badge } from '@/components/ui/badge';

// One KPI as a card: its name (its definition in the hint), its figure for the last period over what it counts (`n` of
// `of`, over how many landings), how that moved from the period before in the server's own reading (better, worse, same, moved,
// too few: the one badge the card wears, and the glyph and words of the move), and a bar for each period of the span from a
// zero baseline, labelled at both ends and at the top, with the figures in words for a screen reader. The figure over the
// bars is named a peak, the tallest period's; the figure the last period reads is under its own bar. The figures and the
// words for the periods are the server's: this formats them, it computes none, and it never decides which way is good
// (OutcomeKpi.change.reads does).
const props = defineProps<{ kpi: OutcomeKpi; span: OutcomeSpan }>();

const HOUR = 3_600_000;
function figure(kind: OutcomeKind, p: OutcomePoint): string {
  if (!p.has) return '—';
  if (kind === 'rate') return Math.round(p.value * 100) + '%';
  if (kind === 'hours') return duration(p.value * HOUR);
  return String(p.value);
}
// What a share or an average is over, so a small sample reads as small; a count is over the period itself.
function sample(kind: OutcomeKind, p: OutcomePoint): string {
  if (!p.has) return 'nothing to count yet';
  if (kind === 'rate') return `${p.n} of ${p.of}`;
  if (kind === 'hours') return `over ${plural(p.of, props.kpi.over)}`;
  return `in ${props.span.last}`;
}
function signed(kind: OutcomeKind, d: number): string {
  const sign = d > 0 ? '+' : d < 0 ? '−' : '';
  const a = Math.abs(d);
  if (kind === 'rate') return `${sign}${Math.round(a * 100)} pts`;
  if (kind === 'hours') return sign + duration(a * HOUR);
  return sign + a;
}
// The server's reading of the move, in a word, the glyph beside it and the colour of both.
const READS: Record<OutcomeReads, { word: string; icon: string }> = {
  better: { word: 'Better', icon: 'check' },
  worse: { word: 'Worse', icon: 'alert' },
  same: { word: 'Same', icon: 'circle' },
  moved: { word: 'Moved', icon: 'dot' },
  few: { word: 'Too few', icon: 'dot' },
  none: { word: 'No compare', icon: 'dot' },
};
const change = computed(() => {
  const r = OUTCOME_LOOK[props.kpi.change.reads];
  const d = READS[props.kpi.change.reads];
  const text = r.text ?? (props.kpi.change.reads === 'same' ? `Same as ${props.span.before}` : `${signed(props.kpi.kind, props.kpi.change.delta)} vs ${props.span.before}`);
  return { tone: r.tone, icon: r.icon ?? d.icon, word: d.word, text };
});
// Each bar's height is its share of the tallest period's (the peak), from zero; a period with nothing to say is a stub.
const top = computed(() => Math.max(...props.kpi.series.map((p) => (p.has ? p.value : 0)), 0));
const bars = computed(() => props.kpi.series.map((p, i) => ({ p, h: p.has && p.value > 0 && top.value > 0 ? Math.max(4, Math.round((p.value / top.value) * 100)) : 0, now: i === props.kpi.series.length - 1 })));
const lastName = computed(() => props.span.last.charAt(0).toUpperCase() + props.span.last.slice(1));
const periodName = (i: number) => (bars.value.length - 1 - i === 0 ? lastName.value : `${plural(bars.value.length - 1 - i, props.span.unit)} before`);
const periodTip = (p: OutcomePoint, i: number) => `${periodName(i)}: ${figure(props.kpi.kind, p)}${p.has && props.kpi.kind !== 'count' ? ` (${sample(props.kpi.kind, p)})` : ''}`;
const peak = computed(() => (top.value > 0 ? figure(props.kpi.kind, { value: top.value, n: 0, of: 0, has: true }) : ''));
// The chart in words, oldest period first, for whoever cannot see the bars.
const said = computed(() => `${props.kpi.label}, last ${plural(bars.value.length, props.span.unit)}, oldest first: ${bars.value.map((b) => figure(props.kpi.kind, b.p)).join(', ')}.${peak.value ? ` Peak ${props.span.unit} ${peak.value}.` : ''}`);
</script>

<template>
  <article class="kpi" :data-kpi="kpi.key" :aria-label="kpi.label">
    <div class="flex items-start gap-2">
      <Tip :text="kpi.hint"><h3 class="min-w-0 flex-1 cursor-help text-[12.5px] leading-[1.3] font-medium text-text2" data-label>{{ kpi.label }}</h3></Tip>
      <Badge :variant="change.tone" size="count" class="mt-px flex-none" data-reads>{{ change.word }}</Badge>
    </div>
    <div class="mt-2 flex items-baseline gap-2">
      <span class="text-[26px] leading-none font-semibold tabular-nums" data-value>{{ figure(kpi.kind, kpi.now) }}</span>
      <span class="min-w-0 truncate text-[11.5px] text-mut tabular-nums" data-n>{{ sample(kpi.kind, kpi.now) }}</span>
    </div>
    <div class="mt-1.5 flex items-center gap-1 text-[11.5px]" :class="toneText[change.tone]" data-change>
      <StandingMark :tone="change.tone" :icon="change.icon" />
      <Tip :text="change.text" when-clipped><span class="truncate">{{ change.text }}</span></Tip>
    </div>
    <figure class="chart mt-3" role="img" :aria-label="said" :title="said">
      <span class="top" aria-hidden="true" data-peak>{{ peak && `peak ${peak}` }}</span>
      <div class="bars" aria-hidden="true">
        <Tip v-for="(b, i) in bars" :key="i" :text="periodTip(b.p, i)">
          <span class="bar"><span class="fill" :class="b.h ? (b.now ? 'bg-blue' : 'bg-line2') : 'bg-line'" :style="{ height: b.h ? b.h + '%' : '2px' }"></span></span>
        </Tip>
      </div>
      <figcaption class="axis" aria-hidden="true"><span>{{ plural(bars.length - 1, span.unit) }} ago</span><span data-now>{{ span.last }} <b v-if="kpi.now.has" class="font-semibold text-text2">{{ figure(kpi.kind, kpi.now) }}</b></span></figcaption>
    </figure>
  </article>
</template>

<style scoped>
/* The card: one surface with its hairline, as every item card is. */
.kpi {
  display: flex;
  flex-direction: column;
  min-width: 0;
  padding: 12px 14px;
  border-radius: 8px;
  background: var(--panel);
  box-shadow: var(--shadow-card);
}
/* The bars rise from a baseline rule, the peak over them at the right, the two ends of time under them with the last period's figure at the right end. */
.chart {
  display: grid;
  grid-template-rows: auto 40px auto;
  margin: 0;
}
.top {
  justify-self: end;
  font-size: 11px;
  color: var(--mut);
  font-variant-numeric: tabular-nums;
  /* A line's height even when empty, so a chart with no peak keeps its baseline level with its neighbours'. */
  min-height: 1.4em;
}
.bars {
  display: flex;
  align-items: flex-end;
  gap: 3px;
  border-bottom: 1px solid var(--line2);
}
.bar {
  display: flex;
  flex: 1 1 0;
  align-items: flex-end;
  height: 100%;
}
.fill {
  width: 100%;
  border-radius: 2px 2px 0 0;
}
.axis {
  display: flex;
  justify-content: space-between;
  margin-top: 3px;
  font-size: 11px;
  color: var(--mut);
  font-variant-numeric: tabular-nums;
}
</style>
