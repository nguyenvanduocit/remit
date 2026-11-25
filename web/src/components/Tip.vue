<script setup lang="ts">
// A hover hint around one element, built on the shadcn-vue Tooltip. It adds no markup of its own, so wrapping an
// element does not change layout. Needs the TooltipProvider that App.vue puts around the signed-in shell.
// A hint that repeats the text its element already shows says nothing: `whenClipped` opens it only while the element cuts
// that text off (a line clamp, a truncate), on itself or on a part of it, measured when the hint would open.
// The hint opens on focus as well as on hover, so its element has to take focus: plain text (a time, a chip) that nothing
// else makes a tab stop gets one here, and a button or link, or text inside one, keeps its own. An element with no text or
// name of its own (a chart's bar, an icon) is named by the hint, or the stop would be announced as nothing. A `whenClipped`
// hint adds no stop: it would put one on every row of a list for text the row already shows. Nor does an element inside an
// `aria-hidden` part (the bars of a chart whose figure already says it all): a focused element must not be hidden from
// assistive technology. Decided when it mounts.
import { onMounted, ref, useTemplateRef } from 'vue';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
const props = defineProps<{ text: string; side?: 'top' | 'right' | 'bottom' | 'left'; whenClipped?: boolean }>();
const trigger = useTemplateRef<{ $el: Element }>('trigger');
const open = ref(false);
const cut = (el: Element) => el.scrollHeight > el.clientHeight || el.scrollWidth > el.clientWidth;
const clips = (el: Element | undefined) => !!el && (cut(el) || [...el.querySelectorAll('*')].some(cut));
const setOpen = (v: boolean) => {
  open.value = v && (!props.whenClipped || clips(trigger.value?.$el));
};

// What already takes the keyboard, itself or around the element; a row or a card that is a roving stop (a RovingList's) counts even
// while it is -1.
const TAKES_KEYBOARD = 'a[href],button,input,select,textarea,summary,[role=button],[role=link],[role=tab],[role=row][tabindex],[data-roving],[tabindex]:not([tabindex="-1"])';
const HIDDEN = '[aria-hidden="true"]';
const stop = ref(false);
const nameless = ref(false);
onMounted(() => {
  const el = trigger.value?.$el;
  if (props.whenClipped || !(el instanceof Element) || el.closest(TAKES_KEYBOARD) || el.closest(HIDDEN) || el.hasAttribute('tabindex')) return;
  stop.value = true;
  nameless.value = !el.textContent?.trim() && !el.hasAttribute('aria-label') && !el.hasAttribute('aria-labelledby') && !el.hasAttribute('role');
});
</script>

<template>
  <Tooltip :open="open" @update:open="setOpen">
    <TooltipTrigger ref="trigger" as-child :tabindex="stop ? 0 : undefined" :role="nameless ? 'img' : undefined" :aria-label="nameless ? text : undefined"><slot /></TooltipTrigger>
    <TooltipContent :side="side" :side-offset="6" class="whitespace-pre-line">{{ text }}</TooltipContent>
  </Tooltip>
</template>
