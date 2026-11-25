<script setup lang="ts">
import { computed, ref, useTemplateRef } from 'vue';
import { useResizeObserver } from '@vueuse/core';
import type { RouteLocationRaw } from 'vue-router';
import { listedChain } from '../catalog';
import type { LearnRole, Memory, Teaching } from '../core/generated/types.generated';
import { fullTime, plural, short } from '../ui';
import Ago from './Ago.vue';
import Icon from './Icon.vue';
import Tip from './Tip.vue';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible';

// One teaching an agent's memory learned from, as its book lists them (MemoryBook): what the librarian was let read of it
// (an accepted card's work, what a teacher said, or a review of what it already holds), whether the librarian has settled
// it, who taught it and when, the chain and card it was taught in (opened there), what was said, kept to a few lines until
// asked for in full, what the librarian wrote of it or why nothing, and the memories it wrote, each resolved by `find`
// within the book and opened by `jump`; `landed` marks the card a link went to. It is one card of a RovingList: it and its
// controls take Tab only while `stop` is its.
const props = defineProps<{
  teaching: Teaching;
  find: (id: string) => Memory | undefined;
  /** Who taught all the book holds: not repeated on the card. */
  teacher?: string;
  stop: boolean;
  landed: boolean;
}>();
const emit = defineEmits<{ jump: [id: string] }>();

const ROLE: Record<LearnRole, { label: string; icon: string }> = {
  accepted: { label: 'Accepted work', icon: 'check' },
  teaching: { label: 'Taught', icon: 'mentor' },
  review: { label: 'Review', icon: 'repeat' },
};
const t = computed(() => props.teaching);
const role = computed(() => ROLE[t.value.role]);
const teacher = computed(() => (t.value.teacher && t.value.teacher !== props.teacher ? t.value.teacher : ''));
const written = computed(() => t.value.written.map((id) => ({ id, memory: props.find(id) })));
const titleOf = (m: Memory) => m.title.trim() || short(m.body, 80) || m.id;
// Where it was taught: its card in its chain, by the chain's title while the replica lists the chain.
const at = computed((): { to: RouteLocationRaw; text: string } | undefined =>
  t.value.chain ? { to: { name: 'chain', params: { id: t.value.chain, tab: 'live' }, query: t.value.card ? { card: t.value.card } : {} }, text: listedChain(t.value.chain)?.title || 'its chain' } : undefined,
);
const tab = computed(() => (props.stop ? 0 : -1));

// What was said shows a few lines; Read it opens the rest, offered only when there is more than those lines hold.
const said = useTemplateRef<{ $el: HTMLElement }>('said');
const clipped = ref(false);
useResizeObserver(
  () => said.value?.$el,
  ([entry]) => {
    const el = entry?.target as HTMLElement | undefined;
    if (el?.dataset.state === 'closed') clipped.value = el.scrollHeight > el.clientHeight + 1;
  },
);
</script>

<template>
  <li class="teaching flex flex-col gap-1.5 rounded-lg px-3 py-2.5" :tabindex="tab" :data-roving="teaching.id" :data-teaching="teaching.id" :data-state="teaching.state">
    <div class="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1 text-xs text-mut">
      <Badge v-if="landed" variant="info" class="flex-none" data-landed><Icon name="arrow-right" :size="11" />Opened from a link</Badge>
      <Badge variant="quiet" class="flex-none" data-role><Icon :name="role.icon" :size="11" />{{ role.label }}</Badge>
      <Badge v-if="teaching.state === 'waiting'" variant="info" class="flex-none" data-teaching-state><Icon name="clock" :size="11" />Waiting for the librarian</Badge>
      <Badge v-else variant="quiet" class="flex-none" data-teaching-state><Icon name="check" :size="11" />Settled</Badge>
      <span v-if="teacher" class="min-w-0 [overflow-wrap:anywhere]" data-teacher>taught by <span class="text-text2">{{ teacher }}</span></span>
      <span>given <Tip :text="fullTime(teaching.createdAt)"><time class="rounded-sm" :datetime="teaching.createdAt"><Ago :at="teaching.createdAt" /></time></Tip></span>
      <span v-if="teaching.settledAt">settled <Tip :text="fullTime(teaching.settledAt)"><time class="rounded-sm" :datetime="teaching.settledAt"><Ago :at="teaching.settledAt" /></time></Tip></span>
    </div>
    <p class="m-0 flex min-w-0 items-baseline gap-1.5 text-xs text-mut" data-source>
      <Icon class="mt-0.5 flex-none self-start" :name="at ? 'chain' : 'memory'" :size="11" />
      <template v-if="at">
        <span class="flex-none">In</span>
        <Tip :text="teaching.sourceLabel"><Button variant="link" size="text" as-child><RouterLink :to="at.to" :tabindex="tab" data-source-link>{{ at.text }}</RouterLink></Button></Tip>
      </template>
      <span v-else class="min-w-0 [overflow-wrap:anywhere]">{{ teaching.sourceLabel || 'no source named' }}</span>
    </p>
    <Collapsible v-if="teaching.text.trim()" v-slot="{ open: reading }" class="flex flex-col gap-1">
      <CollapsibleContent ref="said" force-mount class="said m-0 border-l-2 border-line2 pl-2.5 text-meta text-text2 [overflow-wrap:anywhere]" as="p" data-text>{{ teaching.text }}</CollapsibleContent>
      <CollapsibleTrigger v-if="clipped || reading" as-child>
        <Button variant="link" size="text" :tabindex="tab" :aria-label="`${reading ? 'Hide' : 'Read it'}: what was taught`" data-show-text>{{ reading ? 'Hide' : 'Read it' }}<Icon :name="reading ? 'chevron-up' : 'chevron-down'" :size="11" /></Button>
      </CollapsibleTrigger>
    </Collapsible>
    <p v-if="teaching.note.trim()" class="m-0 flex min-w-0 items-baseline gap-1.5 text-xs text-text2" data-note>
      <Icon class="mt-0.5 flex-none self-start text-mut" name="memory" :size="11" /><span class="min-w-0 [overflow-wrap:anywhere]"><span class="text-mut">The librarian: </span>{{ teaching.note }}</span>
    </p>
    <div v-if="written.length" class="flex min-w-0 flex-col gap-0.5 text-xs" data-written>
      <span class="text-mut">Wrote {{ plural(written.length, 'memory', 'memories') }}</span>
      <ul class="m-0 flex list-none flex-col gap-0.5 p-0">
        <li v-for="w in written" :key="w.id" class="flex min-w-0 items-baseline gap-1.5">
          <Icon class="mt-0.5 flex-none self-start text-mut" name="arrow-right" :size="11" />
          <Button v-if="w.memory" variant="link" size="text" :tabindex="tab" :aria-label="`Go to the memory it wrote: ${titleOf(w.memory)}`" @click="emit('jump', w.id)">{{ titleOf(w.memory) }}</Button>
          <span v-else class="min-w-0 text-mut [overflow-wrap:anywhere]">a memory this book does not hold (<span class="mono">{{ w.id }}</span>)</span>
        </li>
      </ul>
    </div>
    <span v-else-if="teaching.state === 'settled'" class="text-xs text-mut" data-written>Wrote nothing</span>
  </li>
</template>

<style scoped>
/* Scrolled to, it stops below the book's tabs (MemoryBook's --memory-bar). */
li[data-roving] {
  scroll-margin-top: calc(var(--memory-bar, 0px) + var(--space-stack));
}
.teaching {
  background: var(--panel2);
}
/* The card a link went to holds the focus on a lifted surface, beside its "Opened from a link" badge. */
.teaching:focus {
  background: var(--panel3);
}
/* What was said keeps its line breaks, a few lines of it until Read it opens the rest. */
.said {
  white-space: pre-wrap;
}
.said[data-state='closed'] {
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 4;
  overflow: hidden;
}
</style>
