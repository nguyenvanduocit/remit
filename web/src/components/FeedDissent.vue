<script setup lang="ts">
import type { Decision } from '../core/generated/types.generated';
import { fullTime } from '../ui';
import Ago from './Ago.vue';
import Icon from './Icon.vue';
import Markdown from './Markdown.vue';
import Tip from './Tip.vue';

// A second judge's dissent from an answer the Assistant gave for the owner (ADR-0103): a fresh judge re-decided the item
// without seeing the answer and decided otherwise. It sits on the answer, above Overturn, as a warning to read (amber,
// ADR-0090): the words say what it is, the icon repeats it, and its reason is the judge's.
defineProps<{ dissent: Decision }>();
</script>

<template>
  <div class="dissent" data-dissent>
    <div class="flex items-baseline gap-1.5">
      <Icon name="alert" :size="12" />
      <span class="min-w-0 flex-1 [overflow-wrap:anywhere]" data-dissent-text>{{ dissent.text }}</span>
      <Tip :text="fullTime(dissent.createdAt)"><time class="flex-none rounded-sm text-[11px] text-mut" :datetime="dissent.createdAt"><Ago :at="dissent.createdAt" /></time></Tip>
    </div>
    <Markdown v-if="dissent.why" type="quiet" class="why" :text="dissent.why" data-dissent-why />
  </div>
</template>

<style scoped>
.dissent {
  margin-top: 6px;
  padding: 6px 8px;
  border-radius: 6px;
  background: var(--amber-bg);
  color: var(--text);
  font-size: 12px;
}
.dissent .icon {
  color: var(--amber);
}
.why {
  margin-top: 2px;
  font-size: 11.5px;
}
</style>
