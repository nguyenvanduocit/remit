<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import { catalog } from '../catalog';
import { workGroupOf, type CrewBoard } from '../crewColumns';
import type { Playbook } from '../core/generated/types.generated';
import { modelEffort } from '../ui';
import AgentBadge from './AgentBadge.vue';
import ChangeHistory from './ChangeHistory.vue';
import CrewPage from './CrewPage.vue';
import EmptyPage from './EmptyPage.vue';
import EmptyState from './EmptyState.vue';
import KeyValues from './KeyValues.vue';
import Markdown from './Markdown.vue';
import PageBody from './PageBody.vue';
import PageSection from './PageSection.vue';
import PlaybookCard from './PlaybookCard.vue';
import { Badge } from '@/components/ui/badge';
import PageGrid from './PageGrid.vue';

// The playbooks, reference on how kinds of work have gone well (ADR-0113), in the board's shape (CrewPage): every playbook a
// card, in the column of the kind of work it is for; the open one says what it is for, its brief, the cards it suggests,
// and its history. A Chair reads one when it fits its goal; none binds a chain.
const route = useRoute();
const openId = computed(() => (typeof route.params.playbookId === 'string' ? route.params.playbookId : ''));
const pb = computed(() => catalog.playbooks.find((p) => p.id === openId.value));
const board: CrewBoard<Playbook> = { groups: () => catalog.boards?.playbooks ?? [], rows: () => catalog.playbooks, groupOf: workGroupOf, keyOf: (p) => p.id, textOf: (p) => `${p.name} ${p.id} ${p.description}` };

// The agent a card is: its name and page when the library has it, else the plain agent's own word.
function agentOf(id: string) {
  const a = catalog.agents.find((x) => x.id === id);
  return { id, name: a?.name ?? id, page: a ? { name: 'agents', params: { agentId: id } } : undefined };
}
const cards = computed(() => pb.value?.seeds.map((s) => ({ ...s, hired: agentOf(s.agent) })) ?? []);
// Who the cards hire, the library's agents apart from the plain ones: only the first bring instructions and tools of their own.
const hires = computed(() => {
  const all = [...new Set(pb.value?.seeds.map((s) => s.agent))].map(agentOf);
  return { library: all.flatMap((a) => (a.page ? [{ ...a, page: a.page }] : [])), plain: all.filter((a) => !a.page).map((a) => a.name) };
});
</script>

<template>
  <CrewPage group="crew" name="playbooks" :board="board" :open-id="openId" :open="!!pb" noun="playbooks" :title="pb?.name" gone="This playbook is not listed any more.">
    <template #row="{ row, on }"><PlaybookCard :playbook="row" :on="on" /></template>
    <template #empty>
      <EmptyPage title="No playbook yet" icon="spark" say="Write a playbook for fixing a bug: find the cause, fix it with a test first, review it, verify it on the real system">
        <template #what>A playbook is reference: how a kind of work has gone well, in a brief and the cards it suggests. A Chair reads one when it fits its goal and takes what helps; none binds a chain.</template>
        <template #how>
          <li>You tell the Assistant what a playbook should say, and an author chain writes it.</li>
          <li>It is listed here beside the ones that ship with Remit; opening one shows its brief and its cards.</li>
          <li>To have a chain follow one, name it when you ask for the chain; to change it, tell the Assistant again.</li>
        </template>
      </EmptyPage>
    </template>
    <template v-if="pb" #standing>
      <span class="mono flex-none text-mut">{{ pb.id }}</span>
      <span data-badges class="flex flex-none items-center gap-1.5">
        <Badge v-if="pb.builtin" variant="quiet">built in</Badge>
        <Badge v-if="pb.edited" variant="info">edited</Badge>
      </span>
    </template>
    <PageBody v-if="pb">
      <!-- The cards fill the page; wide enough (two cards of a readable width), they stand two abreast, each its own height;
           what is wide in itself (the starting cards, the history) takes the whole row. -->
      <div class="@container">
        <PageGrid>
          <PageSection title="What it is for" id="playbook-about" data-about>
            <p class="mb-3.5 text-text2 [overflow-wrap:anywhere]">{{ pb.description }}</p>
            <KeyValues>
              <dt>Hires</dt>
              <dd>
                <template v-for="(a, i) in hires.library" :key="a.id"><RouterLink class="text-blue-text hover:underline" :to="a.page">{{ a.name }}</RouterLink><template v-if="i < hires.library.length - 1">, </template></template>
                <template v-if="hires.library.length && hires.plain.length">; </template>
                <template v-if="hires.plain.length">plain agents: {{ hires.plain.join(', ') }}</template>
                <template v-if="!cards.length">none</template>
              </dd>
            </KeyValues>
          </PageSection>
          <PageSection title="Brief" id="playbook-brief" data-brief>
            <Markdown type="quiet" :text="pb.brief" />
          </PageSection>
          <PageSection title="Starting cards" :count="cards.length" id="playbook-cards" class="@[1100px]:col-span-2" data-cards>
            <template #aside>A Chair that reads it may take them, skip them or hire others.</template>
            <!-- Two or three cards abreast once each has a line's room, numbered in reading order. -->
            <div v-if="cards.length" class="@container/cards">
              <ol class="m-0 grid list-none gap-3 p-0 @3xl/cards:grid-cols-2 @6xl/cards:grid-cols-3">
                <li v-for="(s, i) in cards" :key="i" class="flex min-w-0 flex-col gap-2 rounded-lg bg-bg2 px-3.5 py-3" data-seed>
                  <div class="flex flex-wrap items-center gap-x-2 gap-y-1">
                    <span class="mono text-mut">{{ i + 1 }}</span>
                    <AgentBadge :agent="s.agent" :size="20" /><b>{{ s.name }}</b>
                    <span v-if="s.hired.name.toLowerCase() !== s.name.toLowerCase()" class="text-mut">· <RouterLink v-if="s.hired.page" class="text-blue-text hover:underline" :to="s.hired.page">{{ s.hired.name }}</RouterLink><template v-else>{{ s.hired.name }}</template></span>
                    <span v-if="s.model || s.effort" class="text-mut">on {{ modelEffort(s.model || 'the Chair\'s model', s.effort) }}</span>
                  </div>
                  <Markdown type="quiet" :text="s.brief" />
                </li>
              </ol>
            </div>
            <EmptyState v-else>No starting cards: the Chair hires from the brief alone.</EmptyState>
          </PageSection>
          <ChangeHistory class="@[1100px]:col-span-2" kind="playbook" :name="pb.id" :rev="pb.rev" />
        </PageGrid>
      </div>
    </PageBody>
  </CrewPage>
</template>
