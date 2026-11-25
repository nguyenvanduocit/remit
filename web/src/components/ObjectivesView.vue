<script setup lang="ts">
import { computed, nextTick, ref, useTemplateRef, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { catalog, chainRecords, projectOf, standingOf } from '../catalog';
import { useClock } from '../clock';
import { useProjectRef, useQueryRef } from '../nav';
import { KEY_RESULT_STATES, keyMove, OBJECTIVE_STATES, objectiveRows, peopleOf, scopesOf, STATUS_FILTERS, type Row, type StatusFilter } from '../objectiveTree';
import { checkText, LOOK, personText, RETIRED } from '../objectiveLook';
import { elbows, INDENT, ROW, ROW_PAD } from '../tree';
import ChainStandingBadge from './ChainStandingBadge.vue';
import EmptyPage from './EmptyPage.vue';
import EmptyState from './EmptyState.vue';
import Icon from './Icon.vue';
import ObjectiveDetail from './ObjectiveDetail.vue';
import PageTabs from './PageTabs.vue';
import ProjectFilter from './ProjectFilter.vue';
import RowMark from './RowMark.vue';
import SearchBox from './SearchBox.vue';
import StandingBadge from './StandingBadge.vue';
import { Button } from '@/components/ui/button';
import { DropdownMenu, DropdownMenuContent, DropdownMenuLabel, DropdownMenuRadioGroup, DropdownMenuRadioItem, DropdownMenuSeparator, DropdownMenuTrigger } from '@/components/ui/dropdown-menu';
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable';

// What the owner wants reached, as a tree table: every objective he approved (a draft marked as one), under it its key results, under
// each the chains that work toward it, one row each, with who answers for it and where it stands (the horizon and the scope are read in the details). The
// server holds and derives all of it (the objective replica carries each key result's status); this page lists it
// (objectiveTree.ts) and, beside it, reads the objective or key result the owner picked (ObjectiveDetail). It reads only: to make or
// change an objective the owner tells the Assistant, whose column sits beside every page.
// An objective names its project, and the page narrows to one as the board does (ProjectFilter, ADR-0108).
// What is picked, what is unfolded, the search and the filters live in the address (/objectives/<id>, ?kr= the key result of it,
// ?open=, ?q=, ?status=, ?scope=, ?project=), so a reload or a link lands on the same screen. A row picks on a click or Enter and folds on its
// chevron or Right and Left; a chain's row goes to the chain. The grid is one tab stop and its rows are walked with the arrow keys.
const route = useRoute();
const router = useRouter();

const query = useQueryRef<string>('q', '');
const status = useQueryRef<StatusFilter>('status', 'all', STATUS_FILTERS);
const scope = useQueryRef<string>('scope', '');
const project = useProjectRef();
const open = useQueryRef<string>('open', '');
const pickedKeyResult = useQueryRef<string>('kr', '');
const openSet = computed(() => new Set(open.value.split(',').filter(Boolean)));
const setOpen = (key: string, on: boolean) => {
  const keys = new Set(openSet.value);
  if (on) keys.add(key);
  else keys.delete(key);
  open.value = [...keys].join(',');
};
// The objective the address names is unfolded once it arrives; after that its row is the owner's to fold. Its key result, when
// `?kr=` names one, is the row that is picked, else the objective's own.
const named = computed(() => (typeof route.params.id === 'string' ? route.params.id : ''));
watch(named, (id) => id && !openSet.value.has(id) && setOpen(id, true), { immediate: true });
const picked = computed(() => (named.value && pickedKeyResult.value ? `${named.value}/${pickedKeyResult.value}` : named.value));

// A horizon passes by the minute, not by the second the clock ticks in.
const clock = useClock();
const minute = computed(() => Math.floor(clock.value.getTime() / 60_000) * 60_000);
const people = computed(() => peopleOf(catalog.employees));
const rows = computed(() => objectiveRows(catalog.objectives, chainRecords.value, people.value, openSet.value, { q: query.value, status: status.value, scope: scope.value, project: project.value }, minute.value));
const scopes = computed(() => scopesOf(catalog.objectives));
const listed = computed(() => catalog.objectives.some((v) => !v.objective.task));
const lines = computed(() => elbows(rows.value));

const lookOf = (r: Exclude<Row, { kind: 'chain' }>) => (r.kind === 'keyResult' && r.retired ? RETIRED : LOOK[r.state]);
const KIND = { objective: 'Objective', keyResult: 'Key result', chain: 'Chain' } as const;
const filterLabel = computed(() => (status.value === 'all' ? '' : LOOK[status.value].label));

const metaOf = (r: Row) => {
  if (r.kind === 'objective') return r.total === 0 ? 'no key results' : `${r.met} of ${r.total} met`;
  if (r.kind !== 'keyResult') return '';
  return `judged by ${personText(r.judge)} · ${checkText(r.check)}${r.stalled ? ' · stalled: takes no new chain' : ''}`;
};
const setStatus = (v: unknown) => {
  if (v) status.value = v as StatusFilter;
};
const setScope = (v: unknown) => {
  scope.value = v === '*' ? '' : String(v);
};

// The keyboard: one row is the tab stop, the arrow keys walk the rows (objectiveTree.ts keyMove).
const grid = useTemplateRef<HTMLElement>('grid');
const focused = ref('');
const tabStop = computed(() => (rows.value.some((r) => r.key === focused.value) ? focused.value : (rows.value[0]?.key ?? '')));
function focusRow(key: string) {
  focused.value = key;
  void nextTick(() => grid.value?.querySelector<HTMLElement>(`[data-key="${CSS.escape(key)}"]`)?.focus());
}
// A row of an objective or a key result is picked in the address; what refines the screen (the search, the filters, what is
// unfolded) goes along.
function pick(row: Exclude<Row, { kind: 'chain' }>) {
  const kr = row.kind === 'keyResult' ? row.keyResultId : undefined;
  void router.push({ name: 'objectives', params: { id: row.objectiveId }, query: { ...route.query, kr } });
}
const close = () => void router.push({ name: 'objectives', query: { ...route.query, kr: undefined } });
function activate(row: Row) {
  if (row.kind === 'chain') void router.push({ name: 'chain', params: { id: row.chainId } });
  else pick(row);
}
function onKey(e: KeyboardEvent, i: number) {
  if (e.target !== e.currentTarget || e.ctrlKey || e.metaKey || e.altKey) return; // a key typed in what is inside the row is that element's
  const move = keyMove(rows.value, i, e.key);
  if (!move) return;
  e.preventDefault();
  if ('focus' in move) focusRow(move.focus);
  else if ('open' in move) setOpen(move.open, true);
  else if ('close' in move) setOpen(move.close, false);
  else activate(rows.value[i]);
}
// Esc puts the details away; in a box with words in it, Esc is the box's.
function onEsc(e: KeyboardEvent) {
  if ((e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLInputElement) && e.target.value) return;
  if (named.value) close();
}
// A cut name says itself whole in a hint; the hint is set as the pointer comes, so 1500 rows carry no tooltip component each.
function hintWhenClipped(e: MouseEvent) {
  const el = (e.target as HTMLElement).closest<HTMLElement>('.title');
  if (el) el.title = el.scrollWidth > el.clientWidth ? (el.textContent ?? '') : '';
}
function onClick(e: MouseEvent, row: Row) {
  focused.value = row.key;
  if (!(e.target as HTMLElement).closest('a, [data-fold]')) activate(row);
}
// The objective the address names comes into view once its row is listed.
watch(
  () => rows.value.some((r) => r.key === named.value),
  (here) => here && void nextTick(() => grid.value?.querySelector(`[data-key="${CSS.escape(named.value)}"]`)?.scrollIntoView({ block: 'nearest' })),
  { immediate: true, flush: 'post' },
);
</script>

<template>
  <section class="flex min-h-0 flex-1 flex-col overflow-hidden" aria-label="Objectives" @keydown.esc="onEsc">
    <PageTabs group="work">
      <template v-if="listed" #end>
        <SearchBox v-model="query" size="sm" class="w-[260px] max-w-full" placeholder="Search objectives…" label="Search objectives" />
        <DropdownMenu>
          <DropdownMenuTrigger as-child>
            <Button class="flex-none" :variant="status === 'all' ? 'outline' : 'secondary'" size="sm" :aria-label="`Filter by status: ${status === 'all' ? 'all' : filterLabel}`" data-status-filter>
              <Icon name="filter" :size="13" /><span v-if="status !== 'all'" class="max-w-28 truncate">{{ filterLabel }}</span>
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" class="w-48">
            <DropdownMenuRadioGroup :model-value="status" @update:model-value="setStatus">
              <DropdownMenuRadioItem value="all">All statuses</DropdownMenuRadioItem>
              <DropdownMenuSeparator />
              <DropdownMenuLabel>Objective</DropdownMenuLabel>
              <DropdownMenuRadioItem v-for="s in OBJECTIVE_STATES" :key="s" :value="s">{{ LOOK[s].label }}</DropdownMenuRadioItem>
              <DropdownMenuSeparator />
              <DropdownMenuLabel>Key result</DropdownMenuLabel>
              <DropdownMenuRadioItem v-for="s in KEY_RESULT_STATES" :key="s" :value="s">{{ LOOK[s].label }}</DropdownMenuRadioItem>
            </DropdownMenuRadioGroup>
          </DropdownMenuContent>
        </DropdownMenu>
        <DropdownMenu v-if="scopes.length">
          <DropdownMenuTrigger as-child>
            <Button class="flex-none" :variant="scope ? 'secondary' : 'outline'" size="sm" :aria-label="`Filter by scope: ${scope || 'all'}`" data-scope-filter>
              <Icon name="list" :size="13" /><span v-if="scope" class="max-w-28 truncate">{{ scope }}</span>
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" class="w-48">
            <DropdownMenuRadioGroup :model-value="scope || '*'" @update:model-value="setScope">
              <DropdownMenuRadioItem value="*">All scopes</DropdownMenuRadioItem>
              <DropdownMenuRadioItem v-for="s in scopes" :key="s" :value="s">{{ s }}</DropdownMenuRadioItem>
            </DropdownMenuRadioGroup>
          </DropdownMenuContent>
        </DropdownMenu>
        <ProjectFilter v-model="project" />
      </template>
    </PageTabs>

    <EmptyPage v-if="!listed" title="No objective yet" icon="chart" say="Make an objective: customers in the game factory get their refunds within a day, and I judge it myself">
      <template #what>An objective is what you want reached, written as key results. Each key result has someone who answers for it, a judge who rules on it, and a check a machine can run. Chains work toward a key result, and this page shows where each stands.</template>
      <template #how>
        <li>You tell the Assistant what you want and who it is for.</li>
        <li>It drafts the objective with its key results; the draft waits in your inbox, and here, marked as a draft.</li>
        <li>You apply it, and chains start working toward each key result. To change it later, tell the Assistant again.</li>
      </template>
    </EmptyPage>
    <ResizablePanelGroup v-else class="min-h-0 flex-1" direction="horizontal" auto-save-id="objectives">
      <ResizablePanel id="objectives-tree" :order="1" :default-size="62" :min-size="35" class="flex">
        <div class="flex min-w-0 flex-1 flex-col px-4 pt-5 pb-3.5">
          <!-- Where the list is narrow (the details open beside it) the Owner column goes, so Status stays whole; the details say who answers. -->
          <div class="@container min-h-0 flex-1 overflow-auto rounded-lg border border-line bg-panel" data-scroll>
            <div class="relative">
              <table ref="grid" role="treegrid" aria-label="Objectives, their key results and the chains that serve them" class="w-full min-w-[540px] @max-[560px]:min-w-[390px] table-fixed border-separate border-spacing-0 text-[13px]">
                <colgroup><col /><col class="w-[150px] @max-[560px]:hidden" /><col class="w-[150px]" /></colgroup>
                <thead>
                  <tr role="row" class="head">
                    <th role="columnheader" scope="col">Objective</th>
                    <th role="columnheader" scope="col" class="@max-[560px]:hidden">Owner</th>
                    <th role="columnheader" scope="col">Status</th>
                  </tr>
                </thead>
                <tbody @mouseover="hintWhenClipped">
                  <tr
                    v-for="(row, i) in rows"
                    :key="row.key"
                    role="row"
                    class="row"
                    :class="{ picked: row.key === picked, retired: row.kind === 'keyResult' && row.retired }"
                    :style="{ height: ROW + 'px' }"
                    :tabindex="row.key === tabStop ? 0 : -1"
                    :data-key="row.key"
                    :data-row="row.kind"
                    :aria-level="row.level + 1"
                    :aria-posinset="row.posinset"
                    :aria-setsize="row.setsize"
                    :aria-expanded="row.expandable ? row.expanded : undefined"
                    :aria-selected="row.key === picked"
                    @keydown="onKey($event, i)"
                    @click="onClick($event, row)"
                    @focus="focused = row.key"
                  >
                    <td role="gridcell" class="name" :style="{ paddingLeft: ROW_PAD + row.level * INDENT + 'px' }">
                      <div class="flex min-w-0 items-center gap-2">
                        <button v-if="row.expandable" type="button" class="hit flex h-5 w-5 flex-none items-center justify-center rounded-[5px] bg-panel3 text-text2 hover:text-text" tabindex="-1" :aria-label="(row.expanded ? 'Fold ' : 'Unfold ') + (row.kind === 'chain' ? row.title : row.text)" data-fold @click="setOpen(row.key, !row.expanded)"><Icon :name="row.expanded ? 'chevron-down' : 'chevron-right'" :size="12" /></button>
                        <RowMark v-else :icon="row.kind === 'objective' ? 'chart' : row.kind === 'keyResult' ? 'circle' : 'link'" />
                        <span class="sr-only">{{ KIND[row.kind] }}:</span>
                        <RouterLink v-if="row.kind === 'chain'" class="title chain" :to="{ name: 'chain', params: { id: row.chainId } }" tabindex="-1">{{ row.title }}</RouterLink>
                        <span v-else class="title" :class="row.kind === 'objective' ? 'font-semibold' : 'font-medium'">{{ row.text }}</span>
                        <span v-if="row.kind === 'objective' && projectOf(row.project)" class="project" :data-project="row.project"><Icon class="flex-none" name="project" :size="11" /><span class="sr-only">Project</span><span class="truncate">{{ projectOf(row.project)?.name }}</span></span>
                        <span v-if="row.kind !== 'chain'" class="meta" :class="{ count: row.kind === 'objective' }">{{ metaOf(row) }}</span>
                      </div>
                    </td>
                    <td role="gridcell" class="cell @max-[560px]:hidden"><span v-if="row.kind !== 'chain'" class="block truncate" :class="{ 'text-mut': row.owner.retired }">{{ personText(row.owner) }}</span></td>
                    <td role="gridcell" class="cell">
                      <ChainStandingBadge v-if="row.kind === 'chain'" :standing="standingOf(row.chainId)" />
                      <StandingBadge v-else :variant="lookOf(row).tone" :icon="lookOf(row).icon">{{ lookOf(row).label }}</StandingBadge>
                    </td>
                  </tr>
                </tbody>
              </table>
              <svg class="lines" :height="rows.length * ROW" aria-hidden="true"><path v-for="(d, i) in lines" :key="i" :d="d" /></svg>
            </div>
            <EmptyState v-if="rows.length === 0" class="px-4 py-6" data-none>No objective matches what you searched or filtered for.</EmptyState>
          </div>
        </div>
      </ResizablePanel>
      <ResizableHandle aria-label="Resize the details" />
      <ResizablePanel id="objectives-detail" :order="2" :default-size="38" :min-size="25" class="flex">
        <ObjectiveDetail :objective="named" :key-result="pickedKeyResult" @close="close" />
      </ResizablePanel>
    </ResizablePanelGroup>
  </section>
</template>

<style scoped>
/* A line of fixed height, so the lines that join the rows follow from the rows alone (tree.ts); the header stays at the top
   of the box while the rows scroll under it. */
.head th {
  position: sticky;
  top: 0;
  z-index: 1;
  height: 32px;
  padding: 0 8px;
  border-bottom: 1px solid var(--line);
  background: var(--bg2);
  color: var(--mut);
  font-size: 11.5px;
  font-weight: 500;
  text-align: left;
}
.head th:first-child {
  padding-left: 12px;
}
.row {
  color: var(--text2);
  cursor: pointer;
}
.row > td {
  overflow: hidden;
  padding: 0 8px;
  white-space: nowrap;
}
.row:hover > td {
  background: var(--panel2);
}
.row.picked > td {
  background: var(--panel3);
}
/* A retired key result is muted in text colour, so what it says stays readable. */
.row.retired > td,
.row.retired .name,
.row.retired .title {
  color: var(--mut);
}
.row:focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: -2px;
}
.name {
  color: var(--text);
}
.title {
  display: block;
  flex: 0 1 auto;
  min-width: 48px;
  overflow: hidden;
  text-overflow: ellipsis;
  text-decoration: none;
}
.title.chain {
  color: var(--text2);
}
.meta {
  flex: 1 1 0;
  min-width: 0;
  overflow: hidden;
  color: var(--mut);
  font-size: 12px;
  text-overflow: ellipsis;
}
/* An objective's project, named beside its outcome and cut at 16 characters (the details name it whole). */
.project {
  display: inline-flex;
  flex: 0 1 auto;
  align-items: center;
  gap: 4px;
  min-width: 0;
  max-width: 16ch;
  color: var(--text2);
  font-size: 12px;
}
/* An objective's count is short and stays whole; the name gives way to it. */
.meta.count {
  flex: none;
}
/* The lines start under the header (32px) and run down the first column. */
.lines {
  position: absolute;
  inset: 32px auto auto 0;
  width: 100%;
  overflow: visible;
  pointer-events: none;
}
.lines path {
  fill: none;
  stroke: var(--tree-line);
  stroke-width: 1.5;
  stroke-linejoin: round;
}
</style>
