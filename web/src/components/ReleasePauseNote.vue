<script setup lang="ts">
import type { ReleasePause } from '../core/generated/types.generated';
import { useStream } from '../streamContext';
import ActorTag from './ActorTag.vue';
import Ago from './Ago.vue';
import Callout from './Callout.vue';

// A repository's last release pause (ADR-0116): held, who pulled it and why; lifted, who lifted it and why too. Landing
// goes on while it holds. The Assistant pulls and lifts it on the owner's word, so this only says how it stands.
defineProps<{ pause: ReleasePause }>();
const stream = useStream();
</script>

<template>
  <Callout v-if="!pause.lifted" tone="note" icon="pause" data-pause="held">
    <template #title>Releases held: changes still land, and the next release waits until the pause is lifted</template>
    <p class="m-0 flex flex-wrap items-center gap-x-1.5 gap-y-1 text-[12.5px] text-text2">
      <ActorTag :by="pause.by" :name-of="stream.nameOf" />
      <span v-if="pause.why" class="min-w-0 [overflow-wrap:anywhere]">{{ pause.why }}</span>
      <span class="text-mut">· <Ago :at="pause.at" /></span>
    </p>
  </Callout>
  <p v-else class="m-0 flex flex-wrap items-center gap-x-1.5 gap-y-1 text-[12.5px] text-mut" data-pause="lifted">
    <span>Releases were held by</span><ActorTag :by="pause.by" :name-of="stream.nameOf" /><span v-if="pause.why" class="[overflow-wrap:anywhere]">({{ pause.why }})</span>
    <span>and lifted by</span><ActorTag :by="pause.lifted.by" :name-of="stream.nameOf" /><span v-if="pause.lifted.why" class="[overflow-wrap:anywhere]">({{ pause.lifted.why }})</span>
    <span>· <Ago :at="pause.lifted.at" /></span>
  </p>
</template>
