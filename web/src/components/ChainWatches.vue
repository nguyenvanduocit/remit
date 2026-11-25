<script setup lang="ts">
import { computed } from 'vue';
import { catalog } from '../catalog';
import Icon from './Icon.vue';
import KeyValues from './KeyValues.vue';
import LineWhen from './LineWhen.vue';
import Notice from './Notice.vue';
import PageSection from './PageSection.vue';
import WatchFacts from './WatchFacts.vue';

// What the chain watches until its effect is verified (ADR-0013), every watch the Signals page lists for it: the key result the
// Chair says it verifies, the signal, the condition that wakes it and the deadline. When the signal's source has gone silent the
// line says why it is held; the deadline still wakes the Chair, which finishes the chain Not done, saying which reading never
// came. Nothing here is edited: the owner tells the Chair to watch something else.
const props = defineProps<{ chainId: string }>();
const watches = computed(() => (catalog.signals?.subscriptions ?? []).filter((l) => l.kind === 'watch' && l.chainId === props.chainId));
</script>

<template>
  <PageSection v-if="watches.length" title="Watching" :count="watches.length" id="watches" data-watches>
    <template #aside><Icon class="text-sand" name="eye" :size="12" aria-hidden="true" />until the effect is verified</template>
    <ul class="m-0 flex list-none flex-col gap-4 p-0">
      <li v-for="w in watches" :key="w.id" class="flex flex-col gap-2" data-watch>
        <p class="m-0 [overflow-wrap:anywhere]" data-what>{{ w.what }}</p>
        <KeyValues>
          <WatchFacts :line="w" />
          <dt>When</dt><dd><LineWhen :line="w" /></dd>
        </KeyValues>
        <Notice v-if="w.held" data-held>{{ w.held }}</Notice>
        <RouterLink class="text-xs text-blue-text hover:underline" :to="{ name: 'subscriptions', params: { entry: w.id } }">On the Subscriptions page</RouterLink>
      </li>
    </ul>
  </PageSection>
</template>
