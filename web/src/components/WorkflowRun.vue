<script setup lang="ts">
import { computed, ref } from 'vue';
import { useAct } from '../act';
import { useSelected } from '../nav';
import type { Card, Workflow } from '../core/generated/types.generated';
import { catalog } from '../catalog';
import { agentName, clockAt } from '../ui';
import { ENGINE_LABEL, ENGINE_TIP, open, resultText, stepAsks, stepLook, VERB_LABEL } from '../workflows';
import { Button } from '@/components/ui/button';
import Ago from './Ago.vue';
import CodeBlock from './CodeBlock.vue';
import ConfirmDialog from './ConfirmDialog.vue';
import Fold from './Fold.vue';
import Icon from './Icon.vue';
import KeyValues from './KeyValues.vue';
import Notice from './Notice.vue';
import PageSection from './PageSection.vue';
import StandingBadge from './StandingBadge.vue';
import StandingMark from './StandingMark.vue';
import StatusChip from './StatusChip.vue';
import Tip from './Tip.vue';
import Until from './Until.vue';

// One run of a workflow script, as the server stored it: who runs it (its engine), where it stands and the phase it is in,
// whose hands it is, every call its script made in order (each with the card it put to work, which opens in the drawer, a
// wait's time or signal, the question it put to the owner), and its result or why it failed. While it has not ended the owner
// can stop it, which stops its cards at work, ends its waits and withdraws its question; its Chair is told.
const props = defineProps<{ run: Workflow; cards: ReadonlyMap<string, Card>; runs: ReadonlyMap<string, Workflow> }>();
const selected = useSelected();
const act = useAct();
const nameOf = (id: string) => {
  const c = props.cards.get(id);
  return c ? agentName(c) : id;
};
// The card a step put to work, named by the employee at work in it (its card's name is the step's own label, as the script
// gave it), else by the card's name.
const workerOf = (id: string) => {
  const c = props.cards.get(id);
  const e = c?.employee ? catalog.employees.find((x) => x.employee.id === c.employee) : undefined;
  return e?.shown ?? nameOf(id);
};
const steps = computed(() => props.run.steps.map((s) => ({ step: s, look: stepLook(s, props.run.status), asks: stepAsks(s) })));
const ended = computed(() => !open(props.run.status));
const hasResult = computed(() => props.run.status === 'done' && props.run.result !== undefined && props.run.result !== null);

const confirmStop = ref(false);
const stopping = ref(false);
async function stop() {
  stopping.value = true;
  if (await act('workflowStop', [props.run.id, ''], 'Workflow stopped')) confirmStop.value = false;
  stopping.value = false;
}
</script>

<template>
  <PageSection :title="run.name" :count="run.steps.length" :id="`run-${run.id}`" :data-run="run.id">
    <template #aside>
      <Tip :text="ENGINE_TIP[run.engine]"><StandingBadge variant="quiet" :icon="run.engine === 'remit' ? 'repeat' : 'terminal'" data-engine>{{ ENGINE_LABEL[run.engine] }}</StandingBadge></Tip>
      <StatusChip :status="run.status" :detail="run.phase" data-status />
      <Button v-if="!ended" variant="outline" size="sm" :aria-label="`Stop the run ${run.name}`" @click="confirmStop = true"><Icon name="stop" :size="12" /> Stop</Button>
    </template>

    <div class="flex flex-col gap-3">
      <KeyValues>
        <dt>Run by</dt>
        <dd><button type="button" class="link" @click="selected = run.card">{{ nameOf(run.card) }}</button></dd>
        <dt>Started</dt>
        <dd><Ago :at="run.createdAt" /></dd>
        <template v-if="run.endedAt"><dt>Ended</dt><dd><Ago :at="run.endedAt" /></dd></template>
        <template v-if="run.resumedFrom"><dt>Resumed from</dt><dd>{{ runs.get(run.resumedFrom)?.name ?? run.resumedFrom }}</dd></template>
      </KeyValues>

      <ol v-if="steps.length" class="m-0 flex list-none flex-col p-0" :aria-label="`Steps of ${run.name}`">
        <li v-for="{ step, look, asks } in steps" :key="step.seq" class="step flex items-start gap-2 py-1.5" :class="{ phase: step.verb === 'phase' }" :data-step="step.seq" :data-verb="step.verb">
          <span class="mt-[3px] flex w-4 flex-none justify-center"><StandingMark :tone="look.tone" :icon="look.icon" :working="look.working" :label="look.label" /></span>
          <span class="seq flex-none">{{ step.seq + 1 }}</span>
          <div class="flex min-w-0 flex-1 flex-col gap-0.5">
            <div class="flex min-w-0 items-baseline gap-2">
              <span class="verb flex-none">{{ VERB_LABEL[step.verb] }}</span>
              <span v-if="step.verb === 'phase'" class="title min-w-0 truncate">{{ asks.text }}</span>
              <span v-else-if="step.label" class="label min-w-0 truncate">{{ step.label }}</span>
              <button v-if="step.card" type="button" class="link min-w-0 truncate" :aria-label="`Open ${nameOf(step.card)}, ${workerOf(step.card)}`" data-card-link @click="selected = step.card">{{ workerOf(step.card) }}</button>
              <span class="ml-auto flex-none text-[11px] text-mut"><Ago :at="step.at" /></span>
            </div>
            <p v-if="asks.at" class="what" data-wait>Until {{ clockAt(asks.at) }}<template v-if="step.status === 'pending'"> · <Until :at="asks.at" /></template></p>
            <p v-else-if="asks.signal" class="what" data-wait>Signal <code>{{ asks.signal.key }}</code><template v-if="asks.signal.condition"> when <code>{{ asks.signal.condition }}</code></template><template v-if="asks.signal.until"> · until {{ clockAt(asks.signal.until) }}</template></p>
            <p v-else-if="asks.text && step.verb !== 'phase'" class="what" :class="{ question: step.verb === 'ask_owner' }" :data-question="step.verb === 'ask_owner' || undefined">{{ asks.text }}</p>
            <Notice v-if="step.error" tone="danger" data-step-error>{{ step.error }}</Notice>
          </div>
        </li>
      </ol>
      <p v-else class="m-0 text-[13px] text-mut">The script has made no call yet.</p>

      <div v-if="hasResult" class="flex flex-col gap-1" data-result>
        <span class="text-[12px] text-mut">Result</span>
        <CodeBlock :text="resultText(run.result)" label="Copy the result" wrap />
      </div>
      <Notice v-if="run.error" :tone="run.status === 'failed' ? 'danger' : 'quiet'" :icon="run.status === 'failed' ? 'alert' : 'stop'" data-error>{{ run.error }}</Notice>

      <Fold>
        <template #summary><span class="text-[12px] text-mut">Script<template v-if="run.claude"> · {{ run.claude.scriptPath }}</template></span></template>
        <CodeBlock class="mt-1.5" :text="run.script" label="Copy the script" />
      </Fold>
    </div>

    <ConfirmDialog
      v-model:open="confirmStop"
      :title="`Stop ${run.name}?`"
      description="Its cards at work stop, its waits end and its question to you is withdrawn. The Chair is told. A run that stopped does not start again."
      irreversible
      verb="Stop run"
      busy="Stopping…"
      :running="stopping"
      @confirm="stop"
    />
  </PageSection>
</template>

<style scoped>
.step + .step {
  border-top: 1px solid var(--line);
}
.step.phase {
  padding-top: 10px;
}
.seq {
  width: 22px;
  margin-top: 1px;
  font-size: 11px;
  color: var(--mut);
  text-align: right;
  font-variant-numeric: tabular-nums;
}
.verb {
  font-size: 11px;
  font-weight: 500;
  color: var(--text2);
}
.phase .verb,
.title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
}
.label {
  font-size: 13px;
  color: var(--text);
}
.link {
  font-size: 12px;
  color: var(--blue-text);
  text-align: left;
  border-radius: 4px;
}
.link:hover {
  text-decoration: underline;
}
.what {
  margin: 0;
  font-size: 12px;
  line-height: 1.45;
  color: var(--text2);
  overflow-wrap: anywhere;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.what.question {
  color: var(--text);
}
.what code {
  font-family: var(--font-mono);
  font-size: 11.5px;
}
</style>
