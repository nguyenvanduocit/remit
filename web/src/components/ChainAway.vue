<script setup lang="ts">
import { computed, ref, useId } from 'vue';
import { useStream } from '../streamContext';
import { hm, plural, span } from '../ui';
import type { Away } from '../core/generated/types.generated';
import ActorTag from './ActorTag.vue';
import Icon from './Icon.vue';
import { Button } from '@/components/ui/button';

// What happened in the chain while the owner was away, over its Live feed as he returns (ADR-0079): how long he was gone, then
// the chain's decisions since he last viewed it, oldest first, each with who made it, and how many more there are, which its
// Overview lists. It is the server's, read once as the chain opened; putting it away is the page's.
const props = defineProps<{ away: Away; chainId: string }>();
const stream = useStream();
const gone = computed(() => span(Date.now() - Date.parse(props.away.since)));
const shown = ref(true);
const headId = useId();
</script>

<template>
  <section v-if="shown" class="away flex max-h-[40%] flex-none flex-col border-b px-4 py-2.5" :aria-labelledby="headId" data-away>
    <header class="flex items-center gap-2">
      <h2 :id="headId" class="text-[12.5px] font-semibold text-blue-hi"><Icon class="mr-1.5" name="clock" :size="12" />While you were away · {{ gone }}</h2>
      <Button variant="ghost" size="icon-xs" class="ml-auto" aria-label="Put away what happened while you were away" @click="shown = false"><Icon name="x" /></Button>
    </header>
    <ol class="mt-1.5 flex min-h-0 flex-col gap-1 overflow-auto text-[12px]">
      <li v-for="(l, i) in away.lines" :key="i" class="flex min-w-0 items-baseline gap-2" data-away-line>
        <time class="flex-none text-[11px] text-mut tabular-nums" :datetime="l.at">{{ hm(l.at) }}</time>
        <ActorTag class="flex-none" :by="l.who" :name-of="stream.nameOf" />
        <span class="min-w-0 text-text2 [overflow-wrap:anywhere]">{{ l.text }}</span>
      </li>
    </ol>
    <RouterLink v-if="away.more" class="mt-1.5 inline-flex w-fit items-center gap-1 text-[12px] text-blue-text hover:underline" :to="{ name: 'chain', params: { id: chainId, tab: 'overview' }, hash: '#decisions' }" data-away-more>{{ plural(away.more, 'more decision') }} in the Overview<Icon name="arrow-right" :size="11" /></RouterLink>
  </section>
</template>

<style scoped>
/* Above the feed, on the owner's blue: news for him, not a run of an agent's work. */
.away {
  border-color: var(--blue-line);
  background: var(--blue-bg);
}
</style>
