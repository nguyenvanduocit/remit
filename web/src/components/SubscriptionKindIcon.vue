<script setup lang="ts">
import { computed } from 'vue';
import type { SubscriptionKind } from '../core/generated/types.generated';
import { kindSaidOf } from '../subscriptionColumns';
import IconChip from './IconChip.vue';

// What kind of subscription a line is, as the server says it (Boards.subscriptionKinds): one icon of its own shape on its kind's
// tint, so a column of rows is told apart at a glance and never by colour alone, named for a screen reader by what the kind
// does; `labelled` writes those words beside it (the page's header), and then the words name it.
const props = defineProps<{ kind: SubscriptionKind; labelled?: boolean }>();
const said = computed(() => kindSaidOf(props.kind));
const icon = computed(() => said.value?.icon ?? 'circle');
const says = computed(() => said.value?.says ?? '');
// The tint of each kind, beside the icon's shape.
const TINT: Record<SubscriptionKind, string> = { check_back: 'bg-blue-bg text-blue-text', watch: 'bg-violet-bg text-violet-text', routine: 'bg-green-bg text-green-text', event: 'bg-sand-bg text-sand', automation: 'bg-orange-bg text-orange-text' };
</script>

<template>
  <span v-if="labelled" class="inline-flex items-center gap-1.5" data-kind><IconChip :icon="icon" :class="TINT[kind]" />{{ says }}</span>
  <IconChip v-else :icon="icon" :class="TINT[kind]" :label="says" data-kind />
</template>
