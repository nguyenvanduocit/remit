<script setup lang="ts">
import { computed } from 'vue';
import type { RouteLocationRaw } from 'vue-router';
import { catalog } from '../catalog';
import type { LearnRole, Memory, MemoryKind, MemoryLinkKind, MemoryScope, Teaching } from '../core/generated/types.generated';
import { fullTime, plural, short } from '../ui';
import Ago from './Ago.vue';
import Icon from './Icon.vue';
import Markdown from './Markdown.vue';
import Tip from './Tip.vue';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible';

// One memory as its holder's book shows it (MemoryBook): its title over what kind it is, whether a person stands behind it,
// how far it reaches, who taught it, when and from which teaching; what it is about; how often it was recalled, how it
// served and how often it was taught again; its body, the evidence it rests on and the memories it bears on, on asking. One
// its holder stopped recalling (superseded, retracted) says so, when, why, and what replaced it. A memory or a teaching it
// names is resolved within the book (`find`, `taughtIn`, `findTeaching`) and opened by `jump` or `teachings`; `landed` marks
// the card a link went to. It is one card of a RovingList: it and its controls take Tab only while `stop` is its. A forget
// is the owner's words to the Assistant, offered only where the caller can say them.
const props = defineProps<{
  memory: Memory;
  find: (id: string) => Memory | undefined;
  /** The teaching it was learned from, when the book lists it. */
  taughtIn: Teaching | undefined;
  findTeaching: (id: string) => Teaching | undefined;
  /** Who taught all the book holds: not repeated on the card. */
  teacher?: string;
  stop: boolean;
  landed: boolean;
  forget?: (m: Memory) => void;
}>();
const emit = defineEmits<{ jump: [id: string]; teachings: [ids: string[]] }>();

const KIND: Record<MemoryKind, { label: string; icon: string }> = {
  lesson: { label: 'Lesson', icon: 'book' },
  pitfall: { label: 'Pitfall', icon: 'alert' },
  preference: { label: 'Preference', icon: 'person' },
  principle: { label: 'Principle', icon: 'scale' },
  decision: { label: 'Decision', icon: 'judge' },
  intent: { label: 'Intent', icon: 'objective' },
  fact: { label: 'Fact', icon: 'file' },
  procedure: { label: 'Procedure', icon: 'list' },
};
const LINK: Record<MemoryLinkKind, string> = {
  because: 'Rests on',
  refines: 'Refines',
  contradicts: 'Contradicts',
  related: 'Related to',
  supersedes: 'Corrects',
};
// What a subject names, by its prefix (`file:`, `tool:`…): its icon, and whether it is a path or a name set in mono.
const SUBJECT: Record<string, { icon: string; mono: boolean }> = {
  file: { icon: 'file', mono: true },
  crate: { icon: 'playbook', mono: true },
  repo: { icon: 'source', mono: true },
  tool: { icon: 'terminal', mono: true },
  topic: { icon: 'comment', mono: false },
  person: { icon: 'person', mono: false },
};
// The teaching it was learned from, in words.
const FROM: Record<LearnRole, string> = { accepted: 'accepted work', teaching: 'a teaching', review: 'a review' };

const m = computed(() => props.memory);
const ended = computed(() => m.value.status !== 'active');
const title = computed(() => m.value.title.trim() || short(m.value.body, 120) || 'A memory with no words');
const kind = computed(() => KIND[m.value.kind]);
const scope = computed(() => {
  const s: MemoryScope = m.value.scope;
  if (s === 'project') return { icon: 'project', text: `Shared in ${catalog.projects.find((p) => p.id === m.value.project)?.name || m.value.project || 'its project'}` };
  if (s === 'lineage') return { icon: 'crew', text: `Shared with every ${m.value.template || 'employee'} hired` };
  if (s === 'everyone') return { icon: 'agent', text: 'Shared with every agent' };
  return { icon: 'person', text: 'Its own' };
});
const teacher = computed(() => (m.value.teacher && m.value.teacher !== props.teacher ? m.value.teacher : ''));
const subjects = computed(() =>
  m.value.about.map((a) => {
    const at = a.indexOf(':');
    const prefix = at > 0 ? a.slice(0, at) : '';
    return { key: a, text: at > 0 ? a.slice(at + 1) : a, ...(SUBJECT[prefix] ?? { icon: 'dot', mono: false }) };
  }),
);
const links = computed(() => m.value.links.map((l) => ({ ...l, label: LINK[l.kind], target: props.find(l.to) })));
const replacedBy = computed(() => (m.value.supersededBy ? props.find(m.value.supersededBy) : undefined));
const retaughtHere = computed(() => m.value.retaught.filter((id) => props.findTeaching(id)));
const titleOf = (t: Memory) => t.title.trim() || short(t.body, 80) || t.id;
const more = computed(() => !!(m.value.body.trim() || m.value.evidence.trim() || links.value.length));
const tab = computed(() => (props.stop ? 0 : -1));
// The work that taught it, when no teaching in the book stands for it: its chain, opened at its card.
const workAt = computed((): RouteLocationRaw | undefined =>
  props.memory.chain ? { name: 'chain', params: { id: props.memory.chain, tab: 'live' }, query: props.memory.card ? { card: props.memory.card } : {} } : undefined,
);
</script>

<template>
  <li class="memory flex flex-col gap-1.5 rounded-lg px-3 py-2.5" :class="{ ended }" :tabindex="tab" :data-roving="memory.id" :data-memory-item="memory.id" :data-status="memory.status">
    <div class="flex min-w-0 items-start gap-2">
      <span class="title min-w-0 flex-1 [overflow-wrap:anywhere]" data-title>{{ title }}</span>
      <Tip v-if="forget" text="Forget it: starts the words in the Assistant's box, and it drafts the forget for you to apply">
        <Button class="-my-0.5 flex-none" variant="ghost" size="icon-xs" :tabindex="tab" :aria-label="`Ask the Assistant to forget the memory ${title}`" data-forget @click="forget(memory)"><Icon name="trash" :size="12" /></Button>
      </Tip>
    </div>

    <div class="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1 text-xs text-mut" data-meta>
      <Badge v-if="landed" variant="info" class="flex-none" data-landed><Icon name="arrow-right" :size="11" />Opened from a link</Badge>
      <Badge variant="quiet" class="flex-none" data-kind><Icon :name="kind.icon" :size="11" />{{ kind.label }}</Badge>
      <Badge v-if="memory.trust === 'confirmed'" variant="success" class="flex-none" data-trust="confirmed"><Icon name="check" :size="11" />Confirmed</Badge>
      <span class="inline-flex min-w-0 items-center gap-1" data-scope><Icon class="flex-none" :name="scope.icon" :size="11" />{{ scope.text }}</span>
      <span v-if="teacher" class="min-w-0 [overflow-wrap:anywhere]" data-teacher>taught by <span class="text-text2">{{ teacher }}</span></span>
      <span class="inline-flex min-w-0 flex-wrap items-baseline gap-x-1" data-created>
        learned <Tip :text="fullTime(memory.createdAt)"><time class="rounded-sm" :datetime="memory.createdAt"><Ago :at="memory.createdAt" /></time></Tip>
        <template v-if="taughtIn">from <Tip :text="memory.sourceLabel"><Button variant="link" size="text" :tabindex="tab" :aria-label="`from ${FROM[taughtIn.role]}: go to the teaching it was learned from`" data-source @click="emit('teachings', [taughtIn.id])">{{ FROM[taughtIn.role] }}</Button></Tip></template>
        <template v-else-if="workAt">from <Tip :text="memory.sourceLabel"><Button variant="link" size="text" as-child><RouterLink :to="workAt" :tabindex="tab" data-source>{{ memory.sourceLabel }}</RouterLink></Button></Tip></template>
        <span v-else-if="memory.sourceLabel" class="min-w-0 [overflow-wrap:anywhere]" data-source>from {{ memory.sourceLabel }}</span>
      </span>
    </div>

    <ul v-if="subjects.length" class="m-0 flex list-none flex-wrap gap-1 p-0" aria-label="What it is about" data-about>
      <li v-for="s in subjects" :key="s.key" class="subject inline-flex max-w-full min-w-0 items-center gap-1 rounded-sm bg-panel3 px-1.5 text-text2" :title="s.key" data-subject>
        <Icon class="flex-none text-mut" :name="s.icon" :size="10" /><span class="min-w-0 [overflow-wrap:anywhere]" :class="{ mono: s.mono }">{{ s.text }}</span>
      </li>
    </ul>

    <div class="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1 text-xs text-mut" data-use>
      <span v-if="memory.recalled" class="inline-flex flex-wrap items-center gap-x-1" data-recalled>
        <Icon name="repeat" :size="11" />Recalled {{ plural(memory.recalled, 'time') }}<template v-if="memory.lastRecalledAt">, last <Tip :text="fullTime(memory.lastRecalledAt)"><time class="rounded-sm" :datetime="memory.lastRecalledAt"><Ago :at="memory.lastRecalledAt" /></time></Tip></template>
      </span>
      <span v-else class="inline-flex items-center gap-1" data-recalled><Icon name="repeat" :size="11" />Not recalled yet</span>
      <span v-if="memory.helped" class="inline-flex items-center gap-1 text-text2" data-helped><Icon name="check" :size="11" />helped {{ memory.helped }}</span>
      <span v-if="memory.misled" class="inline-flex items-center gap-1 text-red-text" data-misled><Icon name="alert" :size="11" />misled {{ memory.misled }}</span>
      <template v-if="memory.retaught.length">
        <Button v-if="retaughtHere.length" variant="link" size="text" :tabindex="tab" :aria-label="`taught again ${plural(memory.retaught.length, 'time')}: go to the teachings that taught it again`" data-retaught @click="emit('teachings', retaughtHere)">taught again {{ plural(memory.retaught.length, 'time') }}</Button>
        <span v-else data-retaught>taught again {{ plural(memory.retaught.length, 'time') }}</span>
      </template>
    </div>

    <div v-if="ended" class="flex min-w-0 flex-col gap-1 text-xs text-text2" data-ended>
      <div class="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-0.5">
        <Badge variant="quiet" class="flex-none" data-ended-status><Icon :name="memory.status === 'superseded' ? 'repeat' : 'x'" :size="11" />{{ memory.status === 'superseded' ? 'Superseded' : 'Retracted' }}</Badge>
        <span v-if="memory.endedAt" class="text-mut"><Tip :text="fullTime(memory.endedAt)"><time class="rounded-sm" :datetime="memory.endedAt"><Ago :at="memory.endedAt" /></time></Tip></span>
      </div>
      <p v-if="memory.endedWhy" class="m-0 min-w-0 [overflow-wrap:anywhere]" data-ended-why>{{ memory.endedWhy }}</p>
      <div v-if="memory.supersededBy" class="flex min-w-0 items-baseline gap-1.5" data-replaced>
        <Icon class="mt-0.5 flex-none self-start text-mut" name="arrow-right" :size="11" />
        <span class="flex-none text-mut">Replaced by</span>
        <Button v-if="replacedBy" variant="link" size="text" :tabindex="tab" :aria-label="`Go to the memory that replaced it: ${titleOf(replacedBy)}`" @click="emit('jump', replacedBy.id)">{{ titleOf(replacedBy) }}</Button>
        <span v-else class="mono text-mut">{{ memory.supersededBy }}</span>
      </div>
    </div>

    <Collapsible v-if="more" v-slot="{ open: reading }">
      <CollapsibleContent class="flex flex-col gap-2 pt-1">
        <Markdown v-if="memory.body.trim()" type="body" class="min-w-0" :text="memory.body" data-body />
        <blockquote v-if="memory.evidence.trim()" class="m-0 border-l-2 border-line2 pl-2.5 text-xs text-text2 [overflow-wrap:anywhere]" data-evidence>
          <span class="mb-0.5 block text-mut">Evidence, in the teaching's words</span>{{ memory.evidence }}
        </blockquote>
        <ul v-if="links.length" class="m-0 flex list-none flex-col gap-1 p-0 text-xs" aria-label="How it bears on other memories" data-links>
          <li v-for="l in links" :key="l.kind + l.to" class="flex min-w-0 items-baseline gap-1.5" :data-link="l.kind">
            <span class="flex-none text-mut">{{ l.label }}</span>
            <Button v-if="l.target" variant="link" size="text" :tabindex="tab" :aria-label="`${l.label}: go to the memory ${titleOf(l.target)}`" @click="emit('jump', l.target.id)">{{ titleOf(l.target) }}</Button>
            <span v-else class="min-w-0 text-mut [overflow-wrap:anywhere]">a memory this book does not hold (<span class="mono">{{ l.to }}</span>)</span>
          </li>
        </ul>
      </CollapsibleContent>
      <CollapsibleTrigger as-child>
        <Button variant="link" size="text" :tabindex="tab" :aria-label="`${reading ? 'Hide' : 'Read it'}: ${title}`" data-show-body>{{ reading ? 'Hide' : 'Read it' }}<Icon :name="reading ? 'chevron-up' : 'chevron-down'" :size="11" /></Button>
      </CollapsibleTrigger>
    </Collapsible>
  </li>
</template>

<style scoped>
/* Scrolled to, it stops below the book's tabs (MemoryBook's --memory-bar). */
li[data-roving] {
  scroll-margin-top: calc(var(--memory-bar, 0px) + var(--space-stack));
}
/* A memory sits on a surface, lighter than the section or drawer under it; whether it is still recalled is its tab, its
   badge and its quieter title. */
.memory {
  background: var(--panel2);
}
.title {
  font-weight: 500;
  color: var(--text);
}
.ended .title {
  color: var(--text2);
}
.subject {
  font-size: 11px;
  line-height: 18px;
}
/* The card a link went to holds the focus on a lifted surface, beside its "Opened from a link" badge. */
.memory:focus {
  background: var(--panel3);
}
</style>
