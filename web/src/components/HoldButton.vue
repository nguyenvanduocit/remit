<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue';
import { usePreferredReducedMotion } from '@vueuse/core';
import { toast } from '../act';
import { Button, type ButtonVariants } from '@/components/ui/button';

// The button of an act that cannot be undone (a high-risk call an agent asks leave for, deleting a chain, stopping a run for
// good): held for `ms` to act, never clicked. Pressed with the pointer (captured, so a hand drifting off the edge still holds)
// or with Enter or Space held down, a bar grows along its foot as the hold goes on, and lets go back to empty when released early.
// Under reduced motion the bar steps in quarters rather than sliding. A plain click (or a tap of the key) acts on nothing and
// says why. It reads "Hold to <label>" (`label`: what it does, "Allow", "Delete chain"), or `busy` while the act runs; its words
// stay the same while held, so the button never shrinks from under the pointer.
const props = withDefaults(defineProps<{ label: string; busy?: string; variant?: ButtonVariants['variant']; size?: ButtonVariants['size']; disabled?: boolean; ms?: number }>(), {
  variant: 'destructive',
  size: 'default',
  ms: 1000,
});
const emit = defineEmits<{ confirm: [] }>();

const progress = ref(0);
const holding = ref(false);
const reduced = usePreferredReducedMotion();
let started = 0;
let frame = 0;
// Whether the press that ends now completed its hold: the click that follows it is then no "plain click".
let done = false;

function tick(now: number) {
  const p = Math.min(1, (now - started) / props.ms);
  progress.value = reduced.value === 'reduce' ? Math.floor(p * 4) / 4 : p;
  if (p >= 1) {
    holding.value = false;
    done = true;
    emit('confirm');
    return;
  }
  frame = requestAnimationFrame(tick);
}
function start() {
  if (props.disabled || holding.value) return;
  holding.value = true;
  done = false;
  started = performance.now();
  frame = requestAnimationFrame(tick);
}
function release() {
  cancelAnimationFrame(frame);
  holding.value = false;
  // A completed hold whose act runs (the caller disables the button meanwhile) keeps its bar full until the act settles.
  if (!(done && props.disabled)) progress.value = 0;
}
function onPointerDown(e: PointerEvent) {
  if (e.button !== 0) return;
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  start();
}
function onKeyDown(e: KeyboardEvent) {
  if (e.key !== 'Enter' && e.key !== ' ') return;
  e.preventDefault();
  if (!e.repeat) start();
}
const explain = () => toast(`Hold “${props.label}” for a second: it cannot be undone`);
function onKeyUp(e: KeyboardEvent) {
  if (e.key !== 'Enter' && e.key !== ' ') return;
  e.preventDefault();
  if (holding.value) explain();
  release();
}
function onClick(e: MouseEvent) {
  if (done) done = false;
  else if (e.detail > 0) explain();
}
// Once the act settles, done or failed, the caller enables the button again and the bar goes back to empty. Sync, so an
// act that settles within the tick it started in is not missed.
watch(
  () => props.disabled,
  (now, was) => {
    if (was && !now && !holding.value) progress.value = 0;
  },
  { flush: 'sync' },
);
onBeforeUnmount(() => cancelAnimationFrame(frame));
</script>

<template>
  <Button
    :variant="variant"
    :size="size"
    :disabled="disabled"
    class="relative isolate overflow-hidden"
    :aria-label="`${label}: hold to confirm`"
    :data-holding="holding || undefined"
    data-hold
    @pointerdown="onPointerDown"
    @pointerup="release"
    @pointercancel="release"
    @lostpointercapture="release"
    @contextmenu="holding && $event.preventDefault()"
    @keydown="onKeyDown"
    @keyup="onKeyUp"
    @blur="release"
    @click="onClick"
  >
    <span class="bar" :class="{ back: !holding }" :style="{ transform: `scaleX(${progress})` }" aria-hidden="true"></span>
    {{ busy ?? `Hold to ${label.charAt(0).toLowerCase()}${label.slice(1)}` }}
  </Button>
</template>

<style scoped>
/* The hold's progress: a bar along the button's foot in the label's own colour, growing from its start edge (a fill behind the
   label would take its contrast below 4.5:1). It returns to empty in a quarter second when let go; the global reduced-motion
   rule makes that return instant. */
.bar {
  position: absolute;
  inset: auto 0 0;
  height: 3px;
  background: currentColor;
  transform-origin: left center;
  pointer-events: none;
}
.bar.back {
  transition: transform 0.25s ease-out;
}
</style>
