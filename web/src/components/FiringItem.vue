<script lang="ts">
import type { ActionKind, FiringLine } from '../core/generated/types.generated';
import type { IconName } from './Icon.vue';

// What a firing did, as an icon beside the server's words wherever a firing is listed: woke a Chair, started a chain, told a
// Chair, hired an agent, ended a goal check, ran a job, told a Chair a mentor did not answer its call for help in time, put an
// item of the owner's to the Assistant to decide first, resumed a step of a workflow run. An action is open (a plugin's
// processor is `plugin:<id>.<name>`, ADR-0049), so one of a plugin's shows an extension's box.
const OWN_ACTION_ICON: Partial<Record<ActionKind, IconName>> = { wake: 'play', start: 'plus', tell: 'comment', hire: 'plus', end: 'x', run: 'play', deadline: 'clock', assist: 'spark', workflow: 'play' };
export const actionIcon = (action: ActionKind): IconName => OWN_ACTION_ICON[action] ?? 'extension';
</script>

<script setup lang="ts">
import { fullTime } from '../ui';
import Ago from './Ago.vue';
import FiringTarget from './FiringTarget.vue';
import Icon from './Icon.vue';

// One firing in words: when, whose subscription (unless the parent already says), what it did with its icon, the chain it acted
// on, and for a deadline that came with no reading to fire it, that it expired. Everything it says is the server's wording.
defineProps<{ firing: FiringLine; titled?: boolean }>();
</script>

<template>
  <li class="flex flex-wrap items-baseline gap-x-2 text-[12px] text-text2" data-firing>
    <span class="w-[72px] flex-none text-mut tabular-nums" :title="fullTime(firing.at)"><Ago :at="firing.at" /></span>
    <slot name="subscription"><b v-if="titled" class="min-w-0 font-semibold [overflow-wrap:anywhere]">{{ firing.title }}</b></slot>
    <span class="flex items-baseline gap-1.5" :class="$slots.subscription ? 'min-w-0 basis-full' : 'min-w-[12rem] flex-1'"><Icon class="self-center text-mut" :name="actionIcon(firing.action)" :size="12" /><span class="min-w-0 [overflow-wrap:anywhere]">{{ firing.outcome }}</span></span>
    <FiringTarget :firing="firing" />
    <span v-if="firing.expired" class="flex items-center gap-1 text-warning" data-expired><Icon name="clock" :size="11" />Deadline passed</span>
  </li>
</template>
