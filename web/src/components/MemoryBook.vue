<script setup lang="ts">
import { computed, nextTick, reactive, ref, useId, useTemplateRef, watch } from 'vue';
import { until, useElementSize } from '@vueuse/core';
import type { Memory, MemoryBook, Teaching } from '../core/generated/types.generated';
import { useQueryRef } from '../nav';
import { useSayToAssistant } from '../sayToAssistant';
import { short } from '../ui';
import ListState from './ListState.vue';
import MemoryCard from './MemoryCard.vue';
import RovingList from './RovingList.vue';
import SearchBox from './SearchBox.vue';
import TeachingCard from './TeachingCard.vue';
import EmptyState from './EmptyState.vue';
import { Badge } from '@/components/ui/badge';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';

// What one agent's memory holds (an employee's, the Assistant's, a mentor's), read-only, as the server orders it: one tab
// for what it recalls now, one for what it once held and stopped recalling (superseded, retracted), one for the teachings
// the librarian learned from. The tabs and a search over the words stay at the top of the scroll; which tab shows and what
// the search holds are view state, in the address under `viewKey` (and `<viewKey>Q`). Each tab's list is one Tab stop the
// arrows cross (RovingList). What one card names (a memory it links to or was replaced by, the teaching it came from or was
// taught again in, what a teaching wrote) opens in its own tab, scrolled to, focused and marked as where the link went. The
// caller reads the book and hands it over; a forget is the owner's words to the Assistant, offered for an employee's book
// (`employee`): the Assistant drafts a forget of an employee's memories only.
const props = defineProps<{
  book: MemoryBook | undefined;
  error: string;
  /** Read, and nothing of this one's held yet. */
  loading: boolean;
  /** Who remembers, as the empty states, the lists' names and a forget say them. */
  who: string;
  /** The address's key for the open tab: one per place a book shows, so two on one screen keep their own. */
  viewKey: string;
  /** The surface the book lies on, which its tabs keep while they stay at the top of the scroll. */
  surface: 'panel' | 'bg2';
  /** The employee whose memory it is, when it is one's: its memories may be forgotten. */
  employee?: string;
  /** Who taught all it holds (a mentor's own book): said by the page, so no card repeats it. */
  teacher?: string;
  /** Reads the book again after a failure (useRemote's retry). */
  retry?: () => void;
}>();

const VIEWS = ['now', 'ended', 'teachings'] as const;
type View = (typeof VIEWS)[number];
const view = useQueryRef<View>(props.viewKey, 'now', VIEWS);
const query = useQueryRef<string>(`${props.viewKey}Q`, '');

const memories = computed(() => props.book?.memories ?? []);
const teachings = computed(() => props.book?.teachings ?? []);
const byId = computed(() => new Map(memories.value.map((m) => [m.id, m])));
const find = (id: string) => byId.value.get(id);
const teachingById = computed(() => new Map(teachings.value.map((t) => [t.id, t])));
const findTeaching = (id: string) => teachingById.value.get(id);
// The teaching a memory was learned from is the one of its holder that carries its source (a teaching is one per holder and
// source: crew's teaching_id), when the book still lists it.
const teachingBySource = computed(() => new Map(teachings.value.map((t) => [`${t.holder}\n${t.source}`, t])));
const taughtIn = (m: Memory) => teachingBySource.value.get(`${m.holder}\n${m.source}`);

// The search keeps the cards whose words hold every word typed, in any case.
const words = computed(() => query.value.toLowerCase().split(/\s+/).filter(Boolean));
const holds = (parts: string[]) => {
  const text = parts.join('\n').toLowerCase();
  return words.value.every((w) => text.includes(w));
};
const shown = (m: Memory) => holds([m.title, m.body, m.evidence, m.source, m.teacher, ...m.about]);
const now = computed(() => memories.value.filter((m) => m.status === 'active' && shown(m)));
const ended = computed(() => memories.value.filter((m) => m.status !== 'active' && shown(m)));
const taught = computed(() => teachings.value.filter((t) => holds([t.text, t.note, t.source, t.teacher])));
const tabs = computed<{ value: View; label: string; count: number }[]>(() => [
  { value: 'now', label: 'Recalls now', count: now.value.length },
  { value: 'ended', label: 'No longer', count: ended.value.length },
  { value: 'teachings', label: 'Teachings', count: taught.value.length },
]);
const searching = computed(() => words.value.length > 0);
const noMatch = computed(() => `Nothing here holds “${query.value.trim()}”.`);

// Where the keyboard stands in each tab's list (RovingList), and the cards the last link went to, marked until the focus
// moves to another card.
const current = reactive<Record<View, string>>({ now: '', ended: '', teachings: '' });
const landed = ref(new Set<string>());
const hint = useId();
const root = useTemplateRef<HTMLElement>('root');
// A card scrolled to (a link's, an arrow's) stops below the tabs that stay over the list, however many lines they wrap to:
// the cards' scroll margin is the bar's height (--memory-bar).
const { height: barHeight } = useElementSize(useTemplateRef<HTMLElement>('bar'), undefined, { box: 'border-box' });

// A card one card names opens in its own tab: the tab is the address's, so the card is focused once the tab it is in shows
// (`target` waits for it). Every tab's list stays mounted (force-mount) and shows by v-show, whose display is set after the
// render that changed the tab: the focus waits for the tick after it.
const target = ref('');
function focusTarget() {
  const el = target.value ? root.value?.querySelector<HTMLElement>(`[data-roving="${CSS.escape(target.value)}"]`) : undefined;
  if (!el) return;
  target.value = '';
  el.scrollIntoView({ block: 'nearest' });
  el.focus({ preventScroll: true });
}
watch(view, () => nextTick(focusTarget), { flush: 'post' });
// A card the search hides is shown by clearing it first: each is a write to the address, and the second must read the
// address the first left, so the tab waits for the search to be cleared.
async function open(tab: View, ids: string[], visible: (id: string) => boolean) {
  if (!ids.length) return;
  landed.value = new Set(ids);
  current[tab] = ids[0]!;
  target.value = ids[0]!;
  if (!ids.every(visible)) {
    query.value = '';
    await until(query).toBe('');
  }
  if (view.value === tab) void nextTick(focusTarget);
  else view.value = tab;
}
function jump(id: string) {
  const m = find(id);
  if (!m) return;
  const tab = m.status === 'active' ? 'now' : 'ended';
  void open(tab, [id], (x) => (tab === 'now' ? now : ended).value.some((s) => s.id === x));
}
function toTeachings(ids: string[]) {
  const held = ids.filter((id) => teachingById.value.has(id));
  void open('teachings', held, (x) => taught.value.some((t) => t.id === x));
}
function onFocusIn(e: FocusEvent) {
  const card = e.target instanceof Element ? e.target.closest<HTMLElement>('[data-roving]') : null;
  if (landed.value.size && !(card?.dataset.roving && landed.value.has(card.dataset.roving))) landed.value = new Set();
}

const sayToAssistant = useSayToAssistant();
const forget = (m: Memory) => sayToAssistant(`Forget memory ${m.id} of ${props.who} (${props.employee}): "${short(m.title || m.body, 120)}"`);
</script>

<template>
  <ListState what="the memory" :held="!loading && (!error || !!book)" :error="error" :empty="!memories.length && !teachings.length" :retry="retry">
    <template #empty><span data-memory-empty>{{ who }} remembers nothing yet. The librarian writes a memory from each teaching: an accepted card's work, a correction from a judge or a mentor, your edits and overturns.</span></template>
    <div ref="root" class="flex flex-col" :style="{ '--memory-bar': `${barHeight}px` }" data-memory-book @focusin="onFocusIn">
      <Tabs v-model="view" class="gap-2">
        <div ref="bar" class="bar sticky top-0 z-10" data-memory-bar>
          <div class="flex flex-wrap items-center gap-2 pb-2" :class="surface === 'panel' ? 'bg-panel' : 'bg-bg2'">
            <TabsList :aria-label="`What ${who} remembers`" class="max-w-full">
              <TabsTrigger v-for="t in tabs" :key="t.value" :value="t.value" class="px-2.5" :data-memory-tab="t.value">
                {{ t.label }}<Badge variant="quiet" size="count" class="tabular-nums">{{ t.count }}</Badge>
              </TabsTrigger>
            </TabsList>
            <SearchBox v-model="query" size="sm" class="ml-auto w-auto max-w-64 min-w-36 flex-1" placeholder="Search memory…" :label="`Search what ${who} remembers`" data-memory-search />
          </div>
        </div>
        <p :id="hint" class="sr-only">Up and down arrows move between the cards of a list; Tab goes on to the controls of the one you are on.</p>
        <TabsContent v-show="view === 'now'" value="now" force-mount :tabindex="-1">
          <RovingList v-if="now.length" v-model:current="current.now" :items="now" :id-of="(m) => m.id" :aria-label="`What ${who} recalls now`" :aria-describedby="hint" data-memory-list>
            <template #default="{ item, stop }">
              <MemoryCard :memory="item" :find="find" :taught-in="taughtIn(item)" :find-teaching="findTeaching" :teacher="teacher" :stop="stop" :landed="landed.has(item.id)" :forget="employee ? forget : undefined" @jump="jump" @teachings="toTeachings" />
            </template>
          </RovingList>
          <EmptyState v-else data-memory-none>{{ searching ? noMatch : ended.length ? `${who} recalls nothing now: every memory it held was corrected or retracted.` : `${who} recalls nothing yet: the librarian has written no memory from its teachings.` }}</EmptyState>
        </TabsContent>
        <TabsContent v-show="view === 'ended'" value="ended" force-mount :tabindex="-1">
          <RovingList v-if="ended.length" v-model:current="current.ended" :items="ended" :id-of="(m) => m.id" :aria-label="`What ${who} no longer recalls`" :aria-describedby="hint" data-memory-list>
            <template #default="{ item, stop }">
              <MemoryCard :memory="item" :find="find" :taught-in="taughtIn(item)" :find-teaching="findTeaching" :teacher="teacher" :stop="stop" :landed="landed.has(item.id)" @jump="jump" @teachings="toTeachings" />
            </template>
          </RovingList>
          <EmptyState v-else data-memory-none>{{ searching ? noMatch : 'Nothing ended yet. A memory a later one corrects, or one whose teaching was overturned, stays here as the record of what was believed when.' }}</EmptyState>
        </TabsContent>
        <TabsContent v-show="view === 'teachings'" value="teachings" force-mount :tabindex="-1">
          <RovingList v-if="taught.length" v-model:current="current.teachings" :items="taught" :id-of="(t: Teaching) => t.id" :aria-label="`What ${who}'s memory learned from`" :aria-describedby="hint" data-teaching-list>
            <template #default="{ item, stop }">
              <TeachingCard :teaching="item" :find="find" :teacher="teacher" :stop="stop" :landed="landed.has(item.id)" @jump="jump" />
            </template>
          </RovingList>
          <EmptyState v-else data-memory-none>{{ searching ? noMatch : 'No teaching yet: an accepted card, a correction or one of your overturns gives the librarian something to learn from.' }}</EmptyState>
        </TabsContent>
      </Tabs>
    </div>
  </ListState>
</template>

<style scoped>
/* A sticky box stops at its scroller's padding, where the cards scrolling by would show above it: while it is stuck, its
   surface reaches up over that padding (the scroller clips what goes past its edge). */
.bar {
  container-type: scroll-state;
}
@container scroll-state(stuck: top) {
  .bar > div::before {
    content: '';
    position: absolute;
    inset: auto 0 100% 0;
    height: var(--space-pane);
    background: inherit;
  }
}
</style>
