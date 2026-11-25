<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import { toast } from '../act';
import { catalog, chainRecords } from '../catalog';
import { useAssistant } from '../nav';
import { askToNotify, canAskToNotify } from '../notify';
import { setSound, soundOn, soundSupported } from '../sound';
import { paletteRows, type PaletteGroup, type PaletteRow } from '../palette';
import { routes } from '../router';
import { useSayToAssistant } from '../sayToAssistant';
import { WAIT_ICON, waitTone } from '../ui';
import { useOpenWait, waiting } from '../waits';
import Elapsed from './Elapsed.vue';
import EmptyState from './EmptyState.vue';
import Icon from './Icon.vue';
import Kbd from './Kbd.vue';
import StandingBadge from './StandingBadge.vue';
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog';

// The palette (⌘K): one box that searches what the browser holds and goes there (palette.ts). What needs the owner comes first,
// in the server's order, each row its kind (the server's word and its icon), what it asks, who it is about and how long it has
// waited; then the places to go; last, once he has typed, "Ask the Assistant", which puts his words in the Assistant's box and
// opens it, for him to send. Enter never changes anything on the server. A modal dialog: the focus stays in it while it is
// open, Escape closes it, and the focus goes back where it was. The box is a combobox over the list: ↑ ↓ move the row that
// Enter opens (marked by its surface and the ↵ beside it), a click opens a row too.
const open = defineModel<boolean>('open', { required: true });
const router = useRouter();
const { show: showAssistant } = useAssistant();
const openWait = useOpenWait();
const sayToAssistant = useSayToAssistant();

const PAGES = routes.flatMap((r) => (typeof r.name === 'string' && typeof r.meta?.tab === 'string' ? [{ name: r.name, label: r.meta.tab }] : []));
const query = ref('');
// Whether the browser may still be asked to notify, read as the palette opens (the owner's answer changes it).
const askable = ref(false);
watch(open, (o) => {
  if (!o) return;
  query.value = '';
  askable.value = canAskToNotify();
});
const found = computed(() =>
  paletteRows(query.value, {
    waiting: waiting.value,
    pages: PAGES,
    chains: chainRecords.value,
    playbooks: catalog.playbooks,
    agents: catalog.agents,
    skills: catalog.skills,
    employees: catalog.employees,
    canAskToNotify: askable.value,
    sound: { supported: soundSupported(), on: soundOn.value },
  }),
);
const rows = computed(() => found.value.rows);
const groups = computed(() => {
  const out: { name: PaletteGroup; rows: PaletteRow[] }[] = [];
  for (const r of rows.value) {
    const last = out[out.length - 1];
    if (last?.name === r.group) last.rows.push(r);
    else out.push({ name: r.group, rows: [r] });
  }
  return out;
});

// The row Enter opens: the first until the owner moves, and the first again whenever what he typed changes the rows.
const active = ref('');
watch(rows, (now) => (active.value = now.some((r) => r.id === active.value) ? active.value : (now[0]?.id ?? '')), { immediate: true });
watch(query, () => (active.value = rows.value[0]?.id ?? ''));
const optionId = (r: PaletteRow) => `palette-${r.id.replace(/[^\w-]/g, '_')}`;
const activeOption = computed(() => {
  const r = rows.value.find((x) => x.id === active.value);
  return r && optionId(r);
});
const list = ref<HTMLElement | null>(null);
function move(step: number) {
  const at = rows.value.findIndex((r) => r.id === active.value);
  const next = rows.value[Math.min(rows.value.length - 1, Math.max(0, at + step))];
  if (!next) return;
  active.value = next.id;
  void nextTick(() => list.value?.querySelector(`#${optionId(next)}`)?.scrollIntoView({ block: 'nearest' }));
}

async function run(r: PaletteRow) {
  const act = r.act;
  if (act.kind === 'notify') {
    const answer = await askToNotify();
    askable.value = canAskToNotify();
    toast(answer === 'granted' ? 'You will be notified when something new needs you' : 'The browser will not show notifications');
    return;
  }
  if (act.kind === 'sound') {
    setSound(act.on);
    toast(act.on ? 'A sound plays when something new needs you' : 'No sound plays when something new needs you');
    return;
  }
  open.value = false;
  if (act.kind === 'wait') openWait(act.wait);
  else if (act.kind === 'go') void router.push(act.to);
  else if (act.kind === 'assistant') showAssistant();
  else sayToAssistant(act.words);
}

function onKey(e: KeyboardEvent) {
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    e.preventDefault();
    move(e.key === 'ArrowDown' ? 1 : -1);
  } else if (e.key === 'Enter' && !e.isComposing) {
    e.preventDefault();
    const r = rows.value.find((x) => x.id === active.value);
    if (r) void run(r);
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="top-[12vh] flex max-h-[76vh] translate-y-0 flex-col gap-0 overflow-hidden p-0 sm:max-w-[640px]" :show-close-button="false" data-palette>
      <DialogTitle class="sr-only">Find</DialogTitle>
      <DialogDescription class="sr-only">Type to find what needs you, a chain or a page. Up and down move, Enter opens. The last row hands your words to the Assistant.</DialogDescription>
      <div class="flex items-center gap-2.5 border-b border-line px-3.5">
        <Icon class="text-mut" name="search" :size="14" />
        <input
          v-model="query"
          class="h-12 min-w-0 flex-1 bg-transparent text-[14px] text-text outline-none placeholder:text-mut"
          role="combobox"
          aria-label="Find"
          aria-expanded="true"
          aria-controls="palette-list"
          aria-autocomplete="list"
          :aria-activedescendant="activeOption"
          placeholder="What needs you, a chain, a page… or ask the Assistant"
          autocomplete="off"
          spellcheck="false"
          data-palette-input
          @keydown="onKey"
        />
        <Kbd>esc</Kbd>
      </div>
      <div id="palette-list" ref="list" role="listbox" aria-label="Results" class="min-h-0 flex-1 overflow-auto p-1.5">
        <div v-for="g in groups" :key="g.name" role="group" :aria-labelledby="`palette-group-${g.name.replace(/\W/g, '')}`" class="mb-1 last:mb-0">
          <div :id="`palette-group-${g.name.replace(/\W/g, '')}`" class="caps px-2 pt-2 pb-1 text-mut" role="presentation">{{ g.name }}</div>
          <div
            v-for="r in g.rows"
            :id="optionId(r)"
            :key="r.id"
            role="option"
            :aria-selected="r.id === active"
            class="row"
            :class="{ sel: r.id === active }"
            :data-row="r.id"
            @click="run(r)"
            @mousemove="active = r.id"
          >
            <template v-if="r.act.kind === 'wait'">
              <StandingBadge class="flex-none" :variant="waitTone(r.act.wait.kind)" :icon="WAIT_ICON[r.act.wait.kind]">{{ r.act.wait.label }}</StandingBadge>
              <span class="min-w-0 flex-1 truncate text-text">{{ r.label }}</span>
              <span class="min-w-0 max-w-[40%] truncate text-[11.5px] text-mut">{{ r.detail }}</span>
              <Elapsed class="flex-none text-[11.5px] text-mut tabular-nums" :from="r.act.wait.since" />
            </template>
            <template v-else>
              <Icon class="flex-none text-mut" :name="r.act.kind === 'ask' ? 'spark' : r.act.kind === 'notify' || r.act.kind === 'sound' ? 'clock' : 'arrow-right'" :size="13" />
              <span class="min-w-0 flex-1 truncate text-text">{{ r.label }}</span>
              <span class="flex-none text-[11.5px] text-mut">{{ r.detail }}</span>
            </template>
            <Kbd class="flex-none" :class="{ invisible: r.id !== active }" aria-hidden="true">↵</Kbd>
          </div>
        </div>
        <p v-if="found.placesHidden" class="m-0 px-2 py-1.5 text-[11.5px] text-mut" data-places-hidden>{{ found.placesHidden }} more places: type to narrow.</p>
        <EmptyState v-if="!rows.length" class="px-2 py-3 text-[12.5px]">Nothing to show. Type to search.</EmptyState>
      </div>
      <footer class="flex flex-wrap items-center gap-x-3.5 gap-y-1 border-t border-line px-3.5 py-2 text-[11px] text-mut">
        <span class="flex items-center gap-1"><Kbd>↑</Kbd><Kbd>↓</Kbd> move</span>
        <span class="flex items-center gap-1"><Kbd>↵</Kbd> open</span>
        <span class="flex items-center gap-1"><Kbd>n</Kbd> next that needs you</span>
        <span class="flex items-center gap-1"><Kbd>g</Kbd><Kbd>b</Kbd> board</span>
        <span class="flex items-center gap-1"><Kbd>g</Kbd><Kbd>a</Kbd> Assistant</span>
        <span class="flex items-center gap-1"><Kbd>?</Kbd> keys</span>
        <span class="ml-auto">Enter never changes anything by itself</span>
      </footer>
    </DialogContent>
  </Dialog>
</template>

<style scoped>
.row {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 34px;
  padding: 5px 8px;
  border-radius: 6px;
  font-size: 12.5px;
  cursor: pointer;
}
.row.sel {
  background: var(--panel3);
}
</style>
