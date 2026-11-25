<script setup lang="ts">
import { useHeadingTag } from '../pageHeading';
import { useSayToAssistant } from '../sayToAssistant';
import Icon from './Icon.vue';
import PageBody from './PageBody.vue';
import PageSection from './PageSection.vue';
import { Button } from '@/components/ui/button';

// A page with nothing on it yet, as a page and not a line: what it is for (`title`, its icon and the `what` slot), how it fills
// (the `how` slot, steps in order) and the one thing the owner says to the Assistant to start it (`say`: the words go in the
// Assistant's box for him to finish). What the page offers besides (its New button) goes in `actions`. The page places it, in
// its main column; it sits in the middle of that column, and from its top when it is taller than the column.
defineProps<{ title: string; icon: string; say?: string }>();
const sayTo = useSayToAssistant();
const Heading = useHeadingTag();
</script>

<template>
  <PageBody class="flex">
    <div class="m-auto flex w-full max-w-3xl flex-col gap-5" data-empty-page>
      <div class="flex flex-col items-center gap-3 text-center">
        <span class="flex h-9 w-9 flex-none items-center justify-center rounded-lg bg-panel2 text-mut" aria-hidden="true"><Icon :name="icon" :size="18" /></span>
        <div class="min-w-0">
          <component :is="Heading" class="text-[18px] leading-snug font-semibold">{{ title }}</component>
          <p class="mt-1.5 text-text2 [overflow-wrap:anywhere]"><slot name="what" /></p>
        </div>
      </div>
      <PageSection title="How it fills" id="empty-how">
        <ol class="steps m-0 flex list-none flex-col gap-2 p-0 text-text2"><slot name="how" /></ol>
      </PageSection>
      <PageSection v-if="say" title="Say to the Assistant" id="empty-say">
        <div class="flex flex-wrap items-center gap-3">
          <q class="min-w-0 text-text2 [overflow-wrap:anywhere]" data-say>{{ say }}</q>
          <Button class="flex-none" variant="outline" size="sm" :aria-label="`Say it to the Assistant: ${say}`" data-say-button @click="sayTo(say!)"><Icon name="spark" :size="12" /> Say it</Button>
        </div>
      </PageSection>
      <div v-if="$slots.actions" class="flex justify-center gap-2"><slot name="actions" /></div>
    </div>
  </PageBody>
</template>

<style scoped>
/* The steps wear their numbers, in the margin before the step's words, so a step reads as one run of text whatever it holds
   (a code chip, a link). */
.steps {
  counter-reset: step;
}
.steps > :deep(li) {
  position: relative;
  padding-left: 28px;
}
.steps > :deep(li)::before {
  content: counter(step);
  counter-increment: step;
  position: absolute;
  top: 0;
  left: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  margin-top: 1px;
  border-radius: 9px;
  background: var(--panel3);
  color: var(--text2);
  font-size: 10.5px;
  font-variant-numeric: tabular-nums;
}
</style>
