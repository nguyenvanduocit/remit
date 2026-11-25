<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { useQueryRef } from '../nav';
import { useApi } from '../core/api';
import { useRemote } from '../useRemote';
import { firstFile, isInfra, treeRows } from '../fileTree';
import { fileList, MAIN_MIN_PERCENT } from '../panels';
import type { Landing } from '../core/generated/types.generated';
import DiffView from './DiffView.vue';
import Icon from './Icon.vue';
import ListState from './ListState.vue';
import PageSection from './PageSection.vue';
import Tip from './Tip.vue';
import { Badge } from '@/components/ui/badge';
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable';

// The files a landing changes, from the base it forked from to the version its gates approved, as Remit's clone holds them
// (landingDiff; a version never changes, so it is read once per landing): the files as an explorer on the left, the diff of
// the one picked (?file=) on the right.
const props = defineProps<{ landing: Landing }>();
const api = useApi();
const diff = useRemote(() => ({ landingId: props.landing.id }), ({ landingId }) => api.landingDiff(landingId));
const files = computed(() => (diff.of.value?.landingId === props.landing.id ? diff.data.value?.files ?? null : null));
const infra = computed(() => isInfra(files.value ?? []));
const closed = ref(new Set<string>());
watch(() => props.landing.id, () => (closed.value = new Set()));
const dir = (p: string) => (p.includes('/') ? p.slice(0, p.lastIndexOf('/') + 1) : '');
const base = (p: string) => p.slice(p.lastIndexOf('/') + 1);

// The explorer. The file shown lives in the address (?file=) so a link lands on it; without one, or when this landing does
// not change it, it is the one firstFile picks.
const rows = computed(() => treeRows(files.value ?? [], closed.value));
const pickedFile = useQueryRef<string>('file', '');
const file = computed(() => {
  const all = files.value ?? [];
  return all.find((f) => f.path === pickedFile.value) ?? firstFile(all);
});
const toggle = (path: string) => {
  const next = new Set(closed.value);
  if (!next.delete(path)) next.add(path);
  closed.value = next;
};
const tree = ref<HTMLElement>();
const pane = ref<HTMLElement>();
// ↑ ↓ walk the files that are showing.
const onKey = (e: KeyboardEvent) => {
  const step = e.key === 'ArrowDown' ? 1 : e.key === 'ArrowUp' ? -1 : 0;
  if (!step) return;
  e.preventDefault();
  const shown = rows.value.filter((r) => r.kind === 'file');
  const at = shown.findIndex((r) => r.path === file.value?.path);
  const next = shown[Math.min(shown.length - 1, Math.max(0, at + step))];
  if (next) pickedFile.value = next.path;
};

// A new file starts at its top, and the explorer keeps it in sight.
watch(() => file.value?.path, () => nextTick(() => {
  if (pane.value) pane.value.scrollTop = 0;
  tree.value?.querySelector('[aria-current]')?.scrollIntoView({ block: 'nearest' });
}));
</script>

<template>
  <PageSection class="flex min-h-0 flex-1 flex-col [&>div:last-child]:flex [&>div:last-child]:min-h-0 [&>div:last-child]:flex-1 [&>div:last-child]:flex-col" title="Files" :count="files?.length" id="landing-files" flush data-files>
    <template #aside><Badge v-if="infra" variant="danger" size="count">Infrastructure change</Badge></template>
    <div class="flex min-h-0 flex-1 flex-col p-3">
      <ListState what="the landing’s files" :held="!!files" :error="diff.error.value" :empty="!!files && !files.length" :retry="diff.retry">
        <template #empty>This version changes no file.</template>
        <ResizablePanelGroup direction="horizontal" auto-save-id="landing-files" class="min-h-80 flex-1">
          <ResizablePanel id="landing-files-tree" :order="1" class="flex" v-bind="fileList">
            <nav ref="tree" class="files" tabindex="0" aria-label="Changed files" @keydown="onKey">
              <div v-for="r in rows" :key="`${r.kind}:${r.path}`" class="row" :class="{ sel: r.kind === 'file' && r.path === file?.path }" :aria-current="r.kind === 'file' && r.path === file?.path ? 'true' : undefined" :style="{ paddingLeft: `${8 + r.depth * 12}px` }" @click="r.kind === 'dir' ? toggle(r.path) : (pickedFile = r.path)">
                <Icon v-if="r.kind === 'dir'" class="w-2.5 text-faint" :name="r.open ? 'chevron-down' : 'chevron-right'" :size="12" />
                <Icon v-else class="text-faint" name="file" :size="12" />
                <Tip :text="r.path" side="right"><span class="mono" :class="{ 'text-text2': r.kind === 'dir' }">{{ r.name }}</span></Tip>
                <span v-if="r.kind === 'file'" class="ml-auto flex gap-1.5 pl-2.5 text-[11px]"><span class="text-green">+{{ r.file.added }}</span><span class="text-red">−{{ r.file.removed }}</span></span>
              </div>
            </nav>
          </ResizablePanel>
          <ResizableHandle class="mx-1 bg-transparent" aria-label="Resize the file list" />
          <ResizablePanel id="landing-files-diff" :order="2" class="flex" :min-size="MAIN_MIN_PERCENT">
            <div ref="pane" class="pane">
              <template v-if="file">
                <div class="sticky top-0 z-[1] flex items-center gap-2 bg-panel px-2.5 py-[7px] text-[12.5px]">
                  <Tip :text="file.path" when-clipped><span class="mono truncate"><span class="text-faint">{{ dir(file.path) }}</span>{{ base(file.path) }}</span></Tip>
                  <Badge v-if="file.new" variant="quiet" size="count">new</Badge><Badge v-if="file.deleted" variant="quiet" size="count">deleted</Badge>
                  <span class="flex-1"></span>
                  <span class="text-green">+{{ file.added }}</span><span class="text-red">−{{ file.removed }}</span>
                </div>
                <DiffView :key="`${landing.id}:${file.path}`" :file="file" />
              </template>
            </div>
          </ResizablePanel>
        </ResizablePanelGroup>
      </ListState>
    </div>
  </PageSection>
</template>

<style scoped>
/* The explorer and the diff scroll on their own, each filling its resizable panel. The diff is a framed pane; the explorer
   is bare rows, as the files of a skill are. */
.files,
.pane {
  position: relative;
  flex: 1;
  min-width: 0;
  overflow: auto;
}
.pane {
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--panel);
}
.files {
  border-radius: 6px;
}
.row {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: max-content;
  padding: 5px 8px;
  border-radius: 6px;
  white-space: nowrap;
  cursor: pointer;
}
.row:hover {
  background: var(--panel2);
}
/* the file open beside the list is pressed into it, as a chosen row is */
.row.sel {
  background: var(--well);
  box-shadow: var(--shadow-pressed);
}
</style>
