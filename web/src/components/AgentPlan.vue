<script setup lang="ts">
import Icon from './Icon.vue';
import { computed } from 'vue';
import type { PlanEntry } from '../core/generated/types.generated';
import { stepLook } from '../stream';
import { plural } from '../ui';
import StandingMark from './StandingMark.vue';

// An agent's plan as it executes: numbered steps, the one in progress highlighted, each one under way or finished marked as
// every step of an agent's work is (stepLook). On its own it is a card with its heading (the drawer's Plan panel); `bare`
// leaves the card and the heading to the section it sits in (a chain's Overview).
const props = defineProps<{ plan: PlanEntry[]; title?: string; bare?: boolean }>();
const done = computed(() => props.plan.filter((p) => p.status === 'completed').length);
</script>

<template>
  <div :class="bare ? '' : 'rounded-lg border border-line bg-panel px-3 py-[11px]'">
    <div v-if="!bare" class="flex items-center gap-1.5 text-[11.5px] text-mut"><Icon name="list" :size="13" /> Plan · {{ plural(plan.length, 'step') }}<span class="ml-auto" :class="{ 'text-success': done === plan.length }">{{ done }}/{{ plan.length }} done</span></div>
    <div v-if="title" class="mt-[5px] mb-1 text-[13px] font-semibold">{{ title }}</div>
    <ol :class="bare ? '' : 'mt-1.5'">
      <li v-for="(p, i) in plan" :key="i" class="step" :class="p.status">
        <span class="num"><StandingMark v-if="p.status === 'in_progress' || p.status === 'completed'" v-bind="stepLook(p.status)" /><template v-else>{{ i + 1 }}</template></span>
        {{ p.content }}
      </li>
    </ol>
  </div>
</template>

<style scoped>
/* Numbered steps: the one in progress lit, finished ones checked. */
.step {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 6px;
  border-radius: 8px;
  font-size: 12.5px;
  color: var(--text2);
}
.step.in_progress {
  background: var(--blue-bg);
  color: var(--text);
}
.step.completed {
  color: var(--mut);
}
.num {
  display: flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  margin-top: -1px;
  border: 1px solid var(--line2);
  border-radius: 50%;
  font-size: 10.5px;
  color: var(--mut);
}
</style>
