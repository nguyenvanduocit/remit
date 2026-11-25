<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import { capabilityLabel, catalog } from '../catalog';
import { sourceGroupOf, type CrewBoard } from '../crewColumns';
import type { ToolSource } from '../core/generated/types.generated';
import { connectionStanding, waitingFor } from '../tools';
import { plural } from '../ui';
import CrewPage from './CrewPage.vue';
import EmptyPage from './EmptyPage.vue';
import Notice from './Notice.vue';
import PageBody from './PageBody.vue';
import PageSection from './PageSection.vue';
import SourceCard from './SourceCard.vue';
import StandingBadge from './StandingBadge.vue';
import { Badge } from '@/components/ui/badge';
import PageGrid from './PageGrid.vue';

// The tools Remit's own agents read the outside world through, in the board's shape (CrewPage): every source a card, one
// column per capability it gives; the open one's page says what it gives, its tools, and its connections (the same code with
// another set of credentials, so one source reaches a staging and a production system) with the variable each needs. A
// secret is never here, only whether the owner has set it: the server keeps the value sealed, and the owner types it into the
// item that asks for it in his inbox. Nothing is edited here: the owner tells the Assistant to connect a source, and the
// items appear.
const route = useRoute();
const openId = computed(() => (typeof route.params.sourceId === 'string' ? route.params.sourceId : ''));
const source = computed(() => catalog.sources.find((s) => s.id === openId.value));
const board: CrewBoard<ToolSource> = {
  groups: () => catalog.boards?.sources ?? [],
  rows: () => catalog.sources,
  groupOf: sourceGroupOf,
  keyOf: (s) => s.id,
  textOf: (s) => `${s.label} ${s.id} ${s.capability} ${s.tools.map((t) => t.name).join(' ')} ${s.connections.map((c) => c.name).join(' ')}`,
};
const waiting = computed(() => (source.value ? waitingFor(catalog.inbox, source.value.id) : 0));
</script>

<template>
  <CrewPage group="integrations" name="tools" :board="board" :open-id="openId" :open="!!source" noun="tools" :title="source?.label" gone="This source is not listed any more.">
    <template #row="{ row, on }"><SourceCard :source="row" :on="on" /></template>
    <template #empty>
      <EmptyPage title="No tool yet" icon="terminal" say="Connect Grafana">
        <template #what>A tool source is an outside system Remit reads for its agents, with tools it wrote for it: a database, a warehouse, the monitoring. A chain that holds the capability a source gives has its tools for every agent; a connection is the source reached with one set of credentials, which the server keeps sealed.</template>
        <template #how>
          <li>You tell the Assistant which source to connect; it raises an item in your inbox for each value the connection needs.</li>
          <li>You type each value into its item: a secret goes into a write-only field, and no agent ever sees it.</li>
          <li>The source is listed here with how it stands; a chain that holds its capability gets its tools.</li>
        </template>
      </EmptyPage>
    </template>
    <template v-if="source" #standing>
      <span data-badges class="flex flex-none items-center gap-1.5">
        <Badge variant="quiet">{{ source.capability }}</Badge>
        <Badge variant="quiet">{{ plural(source.tools.length, 'tool') }}</Badge>
      </span>
    </template>
    <PageBody v-if="source">
      <!-- The cards fill the page; wide enough (two cards of a readable width), they stand two abreast, each its own height. -->
      <div class="@container">
        <PageGrid>
          <PageSection title="What it gives" id="source-gives">
            <p data-gives class="text-text2 [overflow-wrap:anywhere]">
              A chain that holds the capability <code class="mono text-text">{{ source.capability }}</code> ({{ capabilityLabel(source.capability) }}) gives every agent of it these tools.
            </p>
          </PageSection>
          <PageSection title="Tools" :count="source.tools.length" id="source-tools" flush>
            <table class="grid-table" aria-labelledby="source-tools">
              <tbody>
                <tr v-for="t in source.tools" :key="t.name" data-tool-row>
                  <td class="mono text-text">{{ t.name }}</td>
                  <td class="text-text2">{{ t.what }}</td>
                </tr>
              </tbody>
            </table>
          </PageSection>
          <Notice v-if="waiting" tone="needs" icon="key" class="@[1100px]:col-span-2" data-waiting>
            {{ waiting }} {{ waiting === 1 ? 'value waits' : 'values wait' }} for you: type {{ waiting === 1 ? 'it' : 'each' }} into its item in the inbox.
          </Notice>
          <PageSection v-for="c in source.connections" :key="c.name" :title="`Connection ${c.name}`" :id="`connection-${source.id}-${c.name}`" data-connection>
            <template #aside><StandingBadge data-standing :variant="connectionStanding(c).variant" :icon="connectionStanding(c).icon">{{ connectionStanding(c).text }}</StandingBadge></template>
            <p v-if="c.description" data-description class="text-text2 [overflow-wrap:anywhere]">{{ c.description }}</p>
            <p v-else-if="c.name === 'default'" data-description class="text-mut">Served by the server's environment and the values entered for it.</p>
            <Notice v-if="c.problem" tone="needs" icon="alert" data-problem class="mt-2">{{ c.problem }}</Notice>
            <ul v-if="c.variables.length" class="mt-3 border-b border-line" :aria-labelledby="`connection-${source.id}-${c.name}`">
              <li v-for="v in c.variables" :key="v.name" data-variable-row class="border-t border-line py-2">
                <div class="flex flex-wrap items-center gap-x-3 gap-y-1">
                  <span class="mono whitespace-nowrap text-text">{{ v.name }}</span>
                  <span class="whitespace-nowrap text-[11.5px] text-mut">{{ v.secret ? 'secret' : 'plain' }}<template v-if="v.optional"> · optional</template></span>
                  <StandingBadge v-if="v.state === 'unset'" :variant="v.optional ? 'quiet' : 'needs'" icon="key">not set</StandingBadge>
                  <StandingBadge v-else-if="v.state === 'environment'" variant="success" icon="check">from the environment</StandingBadge>
                  <StandingBadge v-else variant="success" icon="check">set</StandingBadge>
                  <span v-if="!v.secret && v.value" data-value class="mono min-w-0 text-text2 [overflow-wrap:anywhere]">{{ v.value }}</span>
                </div>
                <p class="mt-0.5 text-[12px] text-mut">{{ v.what }}</p>
              </li>
            </ul>
          </PageSection>
        </PageGrid>
      </div>
    </PageBody>
  </CrewPage>
</template>

<style scoped>
/* The tools as rows of a flush table: the name in one column, what it answers in the other, a hairline between rows. */
.grid-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}
.grid-table td {
  padding: 8px 16px;
  border-top: 1px solid var(--line);
  vertical-align: top;
}
.grid-table tr:first-child td {
  border-top: 0;
}
.grid-table td:first-child {
  width: 30%;
  overflow-wrap: anywhere;
}
</style>
