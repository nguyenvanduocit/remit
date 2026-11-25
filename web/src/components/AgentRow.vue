<script setup lang="ts">
import Icon from './Icon.vue';
import { computed } from 'vue';
import { catalog } from '../catalog';
import { useSelected } from '../nav';
import { agentName, modelEffort, plural } from '../ui';
import { ROW_BADGE, working, type TreeNode } from '../tree';
import AgentBadge from './AgentBadge.vue';
import AgentStandingBadge from './AgentStandingBadge.vue';
import EmployeeFace from './EmployeeFace.vue';
import HelpBadges from './HelpBadges.vue';
import Tip from './Tip.vue';
import Sheen from './Sheen.vue';
import TreeRow from './TreeRow.vue';
import WorkLine from './WorkLine.vue';

// One agent as a line of an outline (the agent tree of a chain, the steps of an event), `level` steps in: its name and
// model, its employee's face and name, the other agents it works with, what it does now (WorkLine: its work, since when,
// the tool in use, a dot while its time counts as work), its badge, how many turns it took and, when
// agents hang under it, the fold. A click opens the agent.
// A row counts its agent's turns; the Chair's row and a folded one, those of their whole branch, since nothing under them
// shows its own. A row of the Chair's team (`team`) that is over says no more than its badge does (its line only says how
// to reopen it, which its drawer offers); any other agent's line is what it came to (a check's verdict), so it always shows.
// A card a mentor was asked to help, or helps, says so after its name: the mentor's name and where the help stands.
const props = withDefaults(defineProps<{ node: TreeNode; level: number; folded?: boolean; team?: boolean }>(), { folded: false, team: false });
const emit = defineEmits<{ toggle: [] }>();
const selected = useSelected();
const card = computed(() => props.node.card);
const agent = computed(() => props.node.agent);
const isChair = computed(() => card.value.origin === 'chair');
const name = computed(() => (isChair.value ? 'Chair' : card.value.name));
const picked = computed(() => (isChair.value ? selected.value === 'chair' : selected.value === card.value.id));
const pick = () => {
  selected.value = isChair.value ? 'chair' : card.value.id;
};
const doing = computed(() => (props.team && agent.value.doingTone === 'done' ? '' : agent.value.doing));
const said = computed(() => [agent.value.work, doing.value].filter(Boolean).join(' · '));
const branch = computed(() => isChair.value || (props.folded && props.node.children.length > 0));
const turns = computed(() => (branch.value ? agent.value.branch.turns : card.value.turns));
const busy = computed(() => (branch.value ? agent.value.branch.working : working(card.value)));
const under = computed(() => agent.value.branch.agents);
const withNames = computed(() => props.node.with.map((w) => w.name).join(', '));
// A card Claude Code spawned is one of the standing employees at work inside its parent's process: it says whose it is and
// where it runs, since the parent is who the owner talks to.
const employee = computed(() => (card.value.employee ? catalog.employees.find((e) => e.employee.id === card.value.employee) : undefined));
const inside = computed(() => props.node.inside && agentName(props.node.inside));
</script>

<template>
  <TreeRow :level="level" :picked="picked" :line="said" :data-card="card.id" @click="pick">
    <template #mark>
      <AgentBadge :agent="card.agent" :size="ROW_BADGE" />
    </template>
    <template #name>
      <Tip :text="card.name" when-clipped><button type="button" class="block min-h-6 max-w-full truncate text-left leading-6" :aria-pressed="picked" :aria-label="`Open ${isChair ? 'the Chair' : card.name}`">{{ name }}</button></Tip>
    </template>
    <template #meta>
      <span class="model">{{ modelEffort(card.model, card.effort) }}</span>
      <Tip v-if="employee" :text="employee.shown" when-clipped><span class="who" data-employee><EmployeeFace :face="employee.face" :size="14" /><span class="truncate">{{ employee.shown }}</span></span></Tip>
      <span v-if="inside" class="inside" data-inside>inside {{ inside }}</span>
      <HelpBadges :card="card.id" />
      <Tip v-if="node.with.length" :text="'Also works with ' + withNames"><span class="with">+ {{ withNames }}</span></Tip>
    </template>
    <template #line><WorkLine :work="agent.work" :since="agent.since" :warms="agent.warms" :doing="doing" :doing-tone="agent.doingTone" /></template>
    <AgentStandingBadge class="flex-none" :agent="agent" lined />
    <Tip v-if="branch" :text="isChair ? 'turns of the Chair and its team' : 'turns of the whole branch'"><b class="turns"><Sheen :text="plural(turns, 'turn')" :on="busy" /></b></Tip>
    <b v-else class="turns"><Sheen :text="plural(turns, 'turn')" :on="busy" /></b>
    <Tip v-if="node.children.length" :text="(folded ? 'Show ' : 'Hide ') + under + (under === 1 ? ' agent' : ' agents') + ' under ' + name">
      <button type="button" class="fold" :aria-expanded="!folded" :aria-label="(folded ? 'Show ' : 'Hide ') + 'the agents under ' + name" @click.stop="emit('toggle')">{{ under }}<Icon :name="folded ? 'chevron-right' : 'chevron-down'" :size="12" /></button>
    </Tip>
    <span v-else class="fold-space" />
  </TreeRow>
</template>

<style scoped>
.model {
  flex: none;
  font-size: 11px;
  color: var(--mut);
}
.with {
  display: block;
  flex: 0 1 auto;
  min-width: 0;
  max-width: 180px;
  overflow: hidden;
  font-size: 11px;
  color: var(--mut);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.turns {
  flex: none;
  width: 64px;
  font-size: 12px;
  font-weight: 500;
  color: var(--text);
  text-align: right;
  font-variant-numeric: tabular-nums;
}
.fold,
.fold-space {
  flex: none;
  width: 40px;
}
.fold {
  display: inline-flex;
  align-items: center;
  justify-content: flex-end;
  gap: 2px;
  height: 24px;
  padding: 0 4px 0 6px;
  border-radius: 5px;
  color: var(--mut);
  font-size: 11px;
  font-weight: 500;
}
.fold:hover {
  background: var(--veil);
  color: var(--text);
}
/* its employee: the face, then the name cut to fit */
.who {
  display: flex;
  flex: 0 1 auto;
  align-items: center;
  gap: 4px;
  min-width: 0;
  max-width: 160px;
  font-size: 11px;
  color: var(--text2);
  white-space: nowrap;
}
.inside {
  flex: none;
  font-size: 11px;
  color: var(--mut);
}
/* A narrow row (an event's steps beside its drawer) keeps the name, the badge and the turns. */
@container (max-width: 420px) {
  .model,
  .fold-space {
    display: none;
  }
}
</style>
