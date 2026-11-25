<script setup lang="ts">
import { computed } from 'vue';
import { actorName } from '../stream';
import type { Actor } from '../core/generated/types.generated';
import { Badge } from '@/components/ui/badge';

// Who did something on a record (a decision, a write), as a tag in front of what was done: the owner and the Assistant
// acting for him stand out from the agents and Remit. `nameOf` names an agent from its card id.
const props = defineProps<{ by: Actor; nameOf: (cardId: string) => string }>();
const name = computed(() => actorName(props.by, props.nameOf));
const variant = computed(() => (props.by.kind === 'owner' || props.by.kind === 'assistant' ? props.by.kind : 'quiet'));
</script>

<template>
  <Badge :variant="variant" size="count" data-by>{{ name }}</Badge>
</template>
