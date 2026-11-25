<script setup lang="ts">
import { computed } from 'vue';
import type { RouteLocationRaw } from 'vue-router';

// The names an agent holds (its skills), each a link to its page, or "none". A name with no page of its own (`to` gives
// none) shows as itself.
const props = defineProps<{ names: readonly string[]; to: (name: string) => RouteLocationRaw | undefined }>();
const items = computed(() => props.names.map((name) => ({ name, page: props.to(name) })));
</script>

<template>
  <span v-if="!names.length">none</span>
  <span v-else class="mono"><template v-for="(n, i) in items" :key="n.name"><RouterLink v-if="n.page" class="text-blue-text hover:underline" :to="n.page">{{ n.name }}</RouterLink><template v-else>{{ n.name }}</template><template v-if="i < items.length - 1">, </template></template></span>
</template>
