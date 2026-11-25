<script setup lang="ts">
import { computed } from 'vue';
import { feedbackKindOf } from '../feedbackColumns';
import type { FeedbackKind } from '../core/generated/types.generated';
import IconChip from './IconChip.vue';

// The icon of a kind of feedback, as the server names it (Boards.feedback), on its kind's tint, named for a screen reader, so a
// column of cards is told apart at a glance and never by colour alone; `labelled` writes the words beside it (the page's header),
// and then the words name it.
const props = defineProps<{ kind: FeedbackKind; labelled?: boolean }>();
const look = computed(() => feedbackKindOf(props.kind));
const icon = computed(() => look.value?.icon ?? 'circle');
const label = computed(() => look.value?.label ?? '');
// The tint of each kind, beside the icon's shape.
const TINT: Record<FeedbackKind, string> = { practice: 'bg-blue-bg text-blue-text', difficulty: 'bg-amber-bg2 text-amber', complaint: 'bg-red-bg text-red-text', suggestion: 'bg-green-bg text-green-text' };
</script>

<template>
  <span v-if="labelled" class="inline-flex items-center gap-1.5" data-kind><IconChip :icon="icon" :class="TINT[kind]" />{{ label }}</span>
  <IconChip v-else :icon="icon" :class="TINT[kind]" :label="label" data-kind />
</template>
