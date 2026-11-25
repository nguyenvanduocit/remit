<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import { catalog } from '../catalog';
import { workGroupOf, type CrewBoard } from '../crewColumns';
import { allowedText } from '../library';
import type { Agent } from '../core/generated/types.generated';
import AgentCard from './AgentCard.vue';
import ChangeHistory from './ChangeHistory.vue';
import CrewPage from './CrewPage.vue';
import EmptyPage from './EmptyPage.vue';
import EmptyState from './EmptyState.vue';
import KeyValues from './KeyValues.vue';
import Markdown from './Markdown.vue';
import NameLinks from './NameLinks.vue';
import PageBody from './PageBody.vue';
import PageGrid from './PageGrid.vue';
import PageSection from './PageSection.vue';
import { Badge } from '@/components/ui/badge';

// The agents the owner keeps, in the board's shape (CrewPage): every agent a card, in the column of the kind of work it
// is for; the open one says what it is for and exactly what it holds, its instructions, and its history.
const route = useRoute();
const openId = computed(() => (typeof route.params.agentId === 'string' ? route.params.agentId : ''));
const agent = computed(() => catalog.agents.find((a) => a.id === openId.value));
const board: CrewBoard<Agent> = { groups: () => catalog.boards?.agents ?? [], rows: () => catalog.agents, groupOf: workGroupOf, keyOf: (a) => a.id, textOf: (a) => `${a.name} ${a.id} ${a.description} ${a.model ?? ''} ${a.effort ?? ''} ${a.loadout.skills.join(' ')}` };
</script>

<template>
  <CrewPage group="crew" name="agents" :board="board" :open-id="openId" :open="!!agent" noun="agent templates" :title="agent?.name" gone="This template is not listed any more.">
    <template #row="{ row, on }"><AgentCard :agent="row" :on="on" /></template>
    <template #empty>
      <EmptyPage title="No agent template yet" icon="gear" say="Make an agent that reads the production database and answers with numbers, and never writes">
        <template #what>An agent template defines what an agent of this kind is for, the instructions every session of it starts with, the model it runs on, the Claude Code tools it may use and the skills it holds. A Chair hires from these; a card of one takes a copy of what it holds.</template>
        <template #how>
          <li>You tell the Assistant what kind of work an agent should do well, and an author chain writes it.</li>
          <li>It is listed here beside the ones that ship with Remit (the Reviewer and the QC, which sign the gates).</li>
          <li>A playbook's cards may name it, and a Chair hires it by that name; to change it, tell the Assistant again.</li>
        </template>
      </EmptyPage>
    </template>
    <template v-if="agent" #standing>
      <span class="mono flex-none text-mut">{{ agent.id }}</span>
      <span data-badges class="flex flex-none items-center gap-1.5">
        <Badge v-if="agent.builtin" variant="quiet">built in</Badge>
        <Badge v-if="agent.edited" variant="info">edited</Badge>
        <Badge v-if="agent.signs" variant="gate">signs {{ agent.signs }}</Badge>
      </span>
    </template>
    <PageBody v-if="agent">
      <!-- The cards fill the page; wide enough (two cards of a readable width), they stand two abreast, each its own height;
           the history takes the whole row. -->
      <div class="@container">
        <PageGrid>
          <PageSection title="What it is for" id="agent-about" data-about>
            <p class="mb-3.5 text-text2 [overflow-wrap:anywhere]">{{ agent.description }}</p>
            <KeyValues>
              <dt>Model</dt><dd>{{ agent.model || 'worker model' }}</dd>
              <dt>Effort</dt><dd>{{ agent.effort || "the hiring Chair's" }}</dd>
              <dt>Claude Code tools</dt><dd :class="{ mono: agent.loadout.tools?.length }">{{ allowedText(agent.loadout.tools) }}</dd>
              <dt>Skills</dt><dd><NameLinks :names="agent.loadout.skills" :to="(name) => ({ name: 'skills', params: { skillName: name } })" /></dd>
            </KeyValues>
          </PageSection>
          <PageSection title="Instructions" id="agent-instructions" data-instructions>
            <Markdown v-if="agent.loadout.instructions" type="quiet" :text="agent.loadout.instructions" />
            <EmptyState v-else>No instructions of its own: it works from the brief each card is given.</EmptyState>
          </PageSection>
          <ChangeHistory class="@[1100px]:col-span-2" kind="agent" :name="agent.id" :rev="agent.rev" />
        </PageGrid>
      </div>
    </PageBody>
  </CrewPage>
</template>
