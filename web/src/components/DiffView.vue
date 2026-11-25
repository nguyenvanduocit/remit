<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { FileDiff, parsePatchFiles, type FileDiffOptions } from '@pierre/diffs';
import { filePatch } from '../patch';
import type { FileChange } from '../core/generated/types.generated';

// One file's diff, drawn by @pierre/diffs into the shadow DOM of its own <diffs-container>: syntax highlighting, line
// numbers, folded context. The file's header row belongs to the page, so the viewer's own is off. The owner reads it; what he
// wants changed he tells the Chair, on the chain's screen.
const props = defineProps<{ file: FileChange }>();

const patch = computed(() => filePatch(props.file));
const fileDiff = computed(() => (patch.value ? parsePatchFiles(patch.value)[0]?.files[0] : undefined));

// The page's focus ring (style.css) stops at the shadow root, and a code box that scrolls sideways takes the keyboard's focus
// there: inside, the ring is drawn the same way, inset so the box's own edge does not clip it. `--ring` crosses the boundary.
const FOCUS_RING = ':focus-visible { outline: 2px solid var(--ring); outline-offset: -2px; }';
const options: FileDiffOptions<never, never> = {
  theme: 'pierre-dark',
  unsafeCSS: FOCUS_RING,
  diffStyle: 'unified',
  disableFileHeader: true,
};

// The viewer, for as long as there is a diff to draw in the container. It draws when this component tells it to; when it
// needs drawing again by itself (highlighting done, a hunk unfolded) it asks, and is drawn on the next tick.
const host = ref<HTMLElement>();
let view: FileDiff<never> | undefined;
const draw = () => view?.render({ fileDiff: fileDiff.value });
watch(host, (el) => {
  view?.cleanUp();
  view = undefined;
  if (!el || !fileDiff.value) return;
  view = new FileDiff<never>(options, undefined, () => void nextTick(draw));
  view.hydrate({ fileDiff: fileDiff.value, fileContainer: el });
});
watch(fileDiff, draw);
onBeforeUnmount(() => view?.cleanUp());
</script>

<template>
  <div v-if="!patch" class="border-t border-line px-2.5 py-2 text-[11.5px] text-mut">No textual changes to show: a binary file, or one too large to send.</div>
  <diffs-container v-else ref="host" class="block border-t border-line text-[11.5px]" />
</template>
