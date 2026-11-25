<script setup lang="ts">
import { computed, reactive, useTemplateRef } from 'vue';
import { useElementSize } from '@vueuse/core';
import { useRoute, useRouter } from 'vue-router';
import { useApi } from '../core/api';
import { catalog } from '../catalog';
import { employeeOutline, TITLE } from '../employeeTree';
import { useQueryRef } from '../nav';
import { paneStackBelow, stackedList } from '../panels';
import type { Accountable, Employee } from '../core/generated/types.generated';
import { ROW_BADGE } from '../tree';
import { fullTime, plural } from '../ui';
import { useRemote } from '../useRemote';
import Ago from './Ago.vue';
import EmployeeFace from './EmployeeFace.vue';
import EmployeeRuns from './EmployeeRuns.vue';
import EmptyPage from './EmptyPage.vue';
import EmptyState from './EmptyState.vue';
import Icon from './Icon.vue';
import KeyValues from './KeyValues.vue';
import MemoryBook from './MemoryBook.vue';
import Outline from './Outline.vue';
import PageBody from './PageBody.vue';
import PageHead from './PageHead.vue';
import PageSection from './PageSection.vue';
import PageTabs from './PageTabs.vue';
import SearchBox from './SearchBox.vue';
import Tip from './Tip.vue';
import TreeRow from './TreeRow.vue';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { DropdownMenu, DropdownMenuContent, DropdownMenuRadioGroup, DropdownMenuRadioItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu';
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable';

// Who answers to whom among the hired agents, as an outline from `reportsTo` (employeeTree.ts), each by its face
// (EmployeeFace), and beside it the open employee: what it is, its runs across chains, and what it remembers, read-only. The address names the open one (/employees/<id>), the search (?q=)
// and whether the retired show (?retired=show; hidden unless asked for); the replica is the server's. Nothing here changes an employee or a memory: the owner
// tells the Assistant to retire or to forget, and the forget button on a memory only starts those words in its box.
const { width } = useElementSize(useTemplateRef<HTMLElement>('page'));
const stacked = computed(() => width.value > 0 && width.value < paneStackBelow);
const route = useRoute();
const router = useRouter();
const api = useApi();

const openId = computed(() => (typeof route.params.id === 'string' ? route.params.id : ''));
const query = useQueryRef<string>('q', '');
const retiredFilter = useQueryRef<'' | 'show'>('retired', '', ['show']);
const hideRetired = computed(() => retiredFilter.value !== 'show');
const folded = reactive(new Set<string>());
const toggle = (id: string) => (folded.has(id) ? folded.delete(id) : folded.add(id));
const rows = computed(() => employeeOutline(catalog.employees, { folded, hideRetired: hideRetired.value, q: query.value }));
const retired = computed(() => catalog.employees.filter((e) => e.employee.status === 'retired').length);
// The open employee as the server words it (its name, tag and face), and its record.
const openRead = computed(() => catalog.employees.find((e) => e.employee.id === openId.value));
const open = computed(() => openRead.value?.employee);
const labelOf = (id: Accountable) => catalog.employees.find((e) => e.employee.id === id)?.shown ?? id;

const go = (id: string) => void router.push({ name: 'employees', params: { id }, query: route.query });
const close = () => void router.push({ name: 'employees', query: route.query });
const setRetiredFilter = (v: unknown) => (retiredFilter.value = v === 'show' ? 'show' : '');
// Esc closes the open employee; in a box with words in it, Esc is the box's.
const onEsc = (e: KeyboardEvent) => {
  if ((e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLInputElement) && e.target.value) return;
  if (openId.value) close();
};

// What the open employee remembers: read when it opens, and again when its record moves (retired, moved onto a newer
// template). No push carries a memory, so one written while the page is open shows when it is opened again.
const memory = useRemote(
  () => (open.value ? { id: open.value.id, rev: open.value.rev } : undefined),
  (asked) => api.employeeMemory(asked.id),
);
// Its memory book as the server orders it; a reply for the employee open before is not shown under this one.
const book = computed(() => (memory.of.value?.id === open.value?.id ? memory.data.value : undefined));
// Its runs across chains: read when it opens and again when its record moves; no push carries them, so a run that starts
// while the page is open shows when it is opened again.
const runs = useRemote(
  () => (open.value ? { id: open.value.id, rev: open.value.rev } : undefined),
  (asked) => api.employeeRuns(asked.id),
);
const openRuns = computed(() => (runs.of.value?.id === open.value?.id ? runs.data.value : undefined));
const ceilingText = (e: Employee) =>
  [`strongest effect ${e.ceiling.effect || 'not set'}`, plural(e.ceiling.grants?.length ?? 0, 'grant'), `tools: ${e.ceiling.tools?.length ? e.ceiling.tools.join(', ') : 'none'}`].join(' · ');
</script>

<template>
  <section ref="page" class="flex min-h-0 flex-1 flex-col overflow-hidden" aria-label="Employees" @keydown.esc="onEsc">
    <PageTabs group="crew">
      <template v-if="catalog.employees.length" #end>
        <SearchBox v-model="query" size="sm" class="w-[320px] max-w-full" placeholder="Search employees…" label="Search employees" />
        <DropdownMenu v-if="retired">
          <DropdownMenuTrigger as-child>
            <Button class="flex-none" :variant="hideRetired ? 'outline' : 'secondary'" size="sm" :aria-label="`Filter by status: ${hideRetired ? 'active' : 'retired shown'}`" data-retired-filter>
              <Icon name="filter" :size="13" /><span v-if="!hideRetired">Retired shown</span>
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" class="w-48">
            <DropdownMenuRadioGroup :model-value="retiredFilter || 'active'" @update:model-value="setRetiredFilter">
              <DropdownMenuRadioItem value="active">Active employees</DropdownMenuRadioItem>
              <DropdownMenuRadioItem value="show">Show retired ({{ retired }})</DropdownMenuRadioItem>
            </DropdownMenuRadioGroup>
          </DropdownMenuContent>
        </DropdownMenu>
      </template>
    </PageTabs>
    <div v-if="!catalog.employees.length" class="flex min-h-0 min-w-0 flex-1 flex-col">
      <EmptyPage title="No employee yet" icon="key" say="Hire a Chair for the login area of the product, standing, that answers to me">
        <template #what>An employee is an agent you hired from a template and keep until you retire it: a Chair that answers for an objective, a judge that checks its work, a worker that does a part of it. Each has a place in the reporting tree and a memory of its own.</template>
        <template #how>
          <li>You tell the Assistant who to hire, for which area, and who it answers to; it drafts the hire for you to apply.</li>
          <li>A Chair hires workers for its chains; a worker is kept (standing) or let go when its chain ends.</li>
          <li>Here you read who answers to whom and what each one remembers; to retire one or have it forget, tell the Assistant.</li>
        </template>
      </EmptyPage>
    </div>
    <ResizablePanelGroup v-else :key="stacked ? 'stacked' : 'side-by-side'" class="min-h-0 flex-1" :direction="stacked ? 'vertical' : 'horizontal'" :auto-save-id="stacked ? 'employees-stacked' : 'employees'">
      <ResizablePanel id="employees-tree" :order="1" v-bind="stacked && openId ? stackedList : { defaultSize: 55, minSize: 30 }" class="flex">
        <div class="flex min-w-0 flex-1 flex-col">
          <PageBody feed>
            <Outline :rows="rows" label="Who answers to whom">
              <TreeRow v-for="r in rows" :key="r.employee.employee.id" :level="r.level" :picked="r.employee.employee.id === openId" :line="r.employee.employee.scope ?? ''" :data-employee="r.employee.employee.id" @click="go(r.employee.employee.id)">
                <template #mark>
                  <EmployeeFace :face="r.employee.face" :size="ROW_BADGE" />
                </template>
                <template #name>
                  <Tip :text="r.employee.shown" when-clipped>
                    <RouterLink class="flex min-h-6 max-w-full min-w-0 items-center" :class="{ 'text-mut': r.employee.employee.status === 'retired' }" :aria-label="r.employee.shown" :to="{ name: 'employees', params: { id: r.employee.employee.id }, query: route.query }" :aria-current="r.employee.employee.id === openId ? 'true' : undefined" @click.stop>
                      <span v-if="r.employee.employee.label" class="min-w-0 truncate" data-label>{{ r.employee.employee.label }}&nbsp;·&nbsp;</span><span class="min-w-0 truncate" data-tag>{{ r.employee.tag }}</span>
                    </RouterLink>
                  </Tip>
                </template>
                <template #meta>
                  <Badge variant="outline" class="flex-none" data-title><Icon :name="TITLE[r.employee.employee.title].icon" :size="11" />{{ TITLE[r.employee.employee.title].label }}</Badge>
                </template>
                <Badge v-if="r.employee.employee.status === 'retired'" variant="quiet" class="flex-none" data-retired><Icon name="archive" :size="11" />Retired</Badge>
                <Tip v-if="r.under" :text="(folded.has(r.employee.employee.id) ? 'Show ' : 'Hide ') + plural(r.under, 'employee') + ' under ' + r.employee.shown">
                  <button type="button" class="fold" :aria-expanded="!folded.has(r.employee.employee.id)" :aria-label="(folded.has(r.employee.employee.id) ? 'Show ' : 'Hide ') + 'the employees under ' + r.employee.shown" @click.stop="toggle(r.employee.employee.id)">{{ r.under }}<Icon :name="folded.has(r.employee.employee.id) ? 'chevron-right' : 'chevron-down'" :size="12" /></button>
                </Tip>
                <span v-else class="fold-space" />
              </TreeRow>
            </Outline>
            <EmptyState v-if="!rows.length">{{ query.trim() ? 'No employee matches the search.' : 'Every employee is retired.' }}</EmptyState>
          </PageBody>
        </div>
      </ResizablePanel>
      <ResizableHandle v-if="!stacked || openId" aria-label="Resize the employee list" />
      <ResizablePanel v-if="!stacked || openId" id="employees-open" :order="2" :default-size="45" :min-size="30" class="flex">
        <div class="flex min-w-0 flex-1 flex-col">
          <template v-if="openRead && open">
            <PageHead :title="openRead.shown">
              <template #standing>
                <EmployeeFace :face="openRead.face" :size="22" data-open-face />
                <Badge variant="outline" data-title><Icon :name="TITLE[open.title].icon" :size="11" />{{ TITLE[open.title].label }}</Badge>
                <Badge variant="quiet" :data-status="open.status"><Icon :name="open.status === 'retired' ? 'archive' : 'check'" :size="11" />{{ open.status === 'retired' ? 'Retired' : 'Active' }}</Badge>
              </template>
              <template #end><Button variant="ghost" size="icon-sm" aria-label="Close the employee" @click="close"><Icon name="x" :size="14" /></Button></template>
            </PageHead>
            <PageBody>
              <div class="flex flex-col gap-3.5">
                <PageSection title="About" id="employee-about" data-about>
                  <KeyValues>
                    <dt>Answers for</dt><dd>{{ open.scope || 'no area named' }}</dd>
                    <dt>Strong in</dt><dd data-strengths>{{ open.strengths || 'not said yet' }}</dd>
                    <dt>Reports to</dt>
                    <dd>
                      <template v-if="open.reportsTo === 'owner'">you</template>
                      <RouterLink v-else-if="catalog.employees.some((e) => e.employee.id === open!.reportsTo)" class="text-blue-text hover:underline" :to="{ name: 'employees', params: { id: open.reportsTo }, query: route.query }">{{ labelOf(open.reportsTo) }}</RouterLink>
                      <span v-else class="mono">{{ open.reportsTo }}</span>
                    </dd>
                    <dt>Hired by</dt><dd>{{ open.hiredBy === 'owner' ? 'you' : labelOf(open.hiredBy) }}</dd>
                    <dt>Template</dt>
                    <dd>
                      <RouterLink v-if="catalog.agents.some((a) => a.id === open!.template)" class="text-blue-text hover:underline" :to="{ name: 'agents', params: { agentId: open.template } }">{{ open.template }}</RouterLink>
                      <span v-else>{{ open.template }}</span>
                      · revision {{ open.templateRev }}
                    </dd>
                    <dt>May do</dt><dd>{{ ceilingText(open) }}</dd>
                    <dt>Hired</dt><dd><Tip :text="fullTime(open.createdAt)"><time :datetime="open.createdAt"><Ago :at="open.createdAt" /></time></Tip></dd>
                  </KeyValues>
                  <p v-if="open.status === 'retired'" class="mt-3 text-xs text-mut" data-retired-note>Retired: its record and its memory are kept; only a forget you apply removes a memory.</p>
                </PageSection>
                <PageSection title="Runs" id="employee-runs" :count="openRuns?.runs.length" data-runs>
                  <template #aside>newest first</template>
                  <EmployeeRuns :runs="openRuns" :error="runs.error.value" :retry="runs.retry" />
                </PageSection>
                <PageSection title="Memory" id="employee-memory" data-memory>
                  <template #aside>written by the librarian · to forget, tell the Assistant</template>
                  <MemoryBook :book="book" :error="memory.error.value" :loading="memory.loading.value && memory.of.value?.id !== open.id" :who="openRead.shown" view-key="memory" surface="panel" :employee="open.id" :retry="memory.retry" />
                </PageSection>
              </div>
            </PageBody>
          </template>
          <PageBody v-else><EmptyState>{{ openId ? 'This employee is not listed.' : 'Open an employee to read what it remembers.' }}</EmptyState></PageBody>
        </div>
      </ResizablePanel>
    </ResizablePanelGroup>
  </section>
</template>

<style scoped>
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
</style>
