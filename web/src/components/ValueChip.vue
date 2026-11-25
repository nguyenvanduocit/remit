<script setup lang="ts">
import Tip from './Tip.vue';

// A value of a field of an event, as code, the same wherever a rule or an event's page shows one: an empty one says so. `got`: the
// value the event had (green); `other`: one the rule names that the event did not have, read against the one it did.
// Its parent's attributes are the value's, not the hint's.
defineOptions({ inheritAttrs: false });
const props = defineProps<{ value: string; got?: boolean; other?: boolean }>();
const empty = () => props.value === '';
</script>

<template>
  <Tip v-if="got" text="What the event had"><code v-bind="$attrs" class="value got" :class="{ empty: empty() }">{{ empty() ? 'empty' : value }}</code></Tip>
  <code v-else v-bind="$attrs" class="value" :class="{ empty: empty(), other }">{{ empty() ? 'empty' : value }}</code>
</template>

<style scoped>
@reference "../theme.css";

.value {
  @apply code-chip;
  display: inline-block;
  max-width: 100%;
  vertical-align: bottom;
  color: var(--text);
  font-variant-ligatures: none;
  overflow-wrap: anywhere;
}
.value.empty {
  font-family: inherit;
  font-style: italic;
  color: var(--mut);
}
.value.got {
  background: var(--green-tint);
  color: var(--green-text);
}
.value.other {
  background: none;
  color: var(--mut);
}
</style>
