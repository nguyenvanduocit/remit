<script setup lang="ts">
import { computed } from 'vue';
import { catalog } from '../catalog';
import { isPattern, patternWords } from '../pattern';
import type { SignalSource } from '../core/generated/types.generated';
import { patternOf } from '../signalList';

// What a subscription listens to (ADR-0049), as a way back to it on the Signals tab. An exact key goes to its signal's own page (its
// readings, its listeners, why it is silent) when the server's board lists the signal. A pattern goes to the list narrowed to it (the
// search takes the same GLOB) and says in words what it hears. With no key (a subscription to the clock), the list narrowed to the key
// shape of its source, which says what time is.
const props = defineProps<{ signalKey?: string; source: SignalSource }>();
const pattern = computed(() => !!props.signalKey && isPattern(props.signalKey));
const signal = computed(() => (props.signalKey && !pattern.value ? catalog.signals?.signals.find((s) => s.key === props.signalKey) : undefined));
const source = computed(() => catalog.signals?.sources.find((s) => s.source === props.source));
const to = computed(() => {
  if (signal.value) return { name: 'signals', params: { entry: signal.value.id } };
  if (pattern.value) return { name: 'signals', query: { q: props.signalKey } };
  return { name: 'signals', query: { q: source.value ? patternOf(source.value) : undefined } };
});
const words = computed(() => (pattern.value && props.signalKey ? patternWords(props.signalKey, catalog.signals?.sources ?? [], catalog.signals?.signals.map((s) => s.key) ?? []) : ''));
</script>

<template>
  <RouterLink v-if="signal || source || pattern" class="text-blue-text hover:underline" :class="{ mono: signalKey }" :to="to" data-signal-link>{{ signalKey ?? source!.title }}</RouterLink>
  <span v-else class="mono">{{ signalKey }}</span>
  <span v-if="words" class="ml-1.5 text-xs text-mut" data-pattern-words>· {{ words }}</span>
</template>
