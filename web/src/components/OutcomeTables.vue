<script setup lang="ts">
import PageSection from './PageSection.vue';
import type { OutcomeReport } from '../core/generated/types.generated';
import { modelEffort } from '../ui';
import { Table, TableBody, TableCell, TableEmpty, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';

// Everything recorded, by role, by model and by kind of chain, and what the agents used, by tool, MCP tool and skill: the
// server's counts as they are, so the owner sees which kind of agent, which model and which way of working does well, and
// what they reach for. One section, its tabs in the heading, the table flush with the card's edges.
defineProps<{ report: OutcomeReport }>();
const num = 'text-right tabular-nums';
const usage = [
  { value: 'tools', label: 'By tool', head: 'Tool', rows: (r: OutcomeReport) => r.tools, empty: 'No tool call has been counted yet. Calls made before tool names were kept are not counted here.' },
  { value: 'mcp', label: 'By MCP', head: 'MCP tool', rows: (r: OutcomeReport) => r.mcp, empty: 'No agent has used an MCP tool yet.' },
  { value: 'skills', label: 'By skill', head: 'Skill', rows: (r: OutcomeReport) => r.skills, empty: 'No agent has loaded a skill yet.' },
];
</script>

<template>
  <Tabs default-value="roles">
    <PageSection title="Everything recorded" id="tables" flush data-tables>
      <template #aside>
        <TabsList variant="line" class="h-8 gap-4">
          <TabsTrigger value="roles">By role</TabsTrigger>
          <TabsTrigger value="models">By model</TabsTrigger>
          <TabsTrigger value="kinds">By kind</TabsTrigger>
          <TabsTrigger v-for="u in usage" :key="u.value" :value="u.value">{{ u.label }}</TabsTrigger>
        </TabsList>
      </template>
      <TabsContent value="roles">
        <Table class="text-[12.5px]">
          <TableHeader>
            <TableRow><TableHead class="pl-4">Role</TableHead><TableHead :class="num">Agents</TableHead><TableHead :class="num">Failed</TableHead><TableHead :class="num">Turns</TableHead><TableHead :class="num">Accepted</TableHead><TableHead :class="num">Sent back</TableHead><TableHead :class="num">Approved</TableHead><TableHead :class="num" class="pr-4">Changes asked</TableHead></TableRow>
          </TableHeader>
          <TableBody>
            <TableEmpty v-if="!report.roles.length" :colspan="8" class="text-mut">No agents have worked yet.</TableEmpty>
            <TableRow v-for="r in report.roles" :key="r.role">
              <TableCell class="pl-4 font-medium">{{ r.role }}</TableCell><TableCell :class="num">{{ r.agents }}</TableCell><TableCell :class="num">{{ r.failed }}</TableCell><TableCell :class="num">{{ r.turns }}</TableCell><TableCell :class="num">{{ r.accepted }}</TableCell><TableCell :class="num">{{ r.sentBack }}</TableCell><TableCell :class="num">{{ r.approved }}</TableCell><TableCell :class="num" class="pr-4">{{ r.changesRequested }}</TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </TabsContent>
      <TabsContent value="models">
        <Table class="text-[12.5px]">
          <TableHeader>
            <TableRow><TableHead class="pl-4">Model</TableHead><TableHead :class="num">Agents</TableHead><TableHead :class="num">Failed</TableHead><TableHead :class="num" class="pr-4">Turns</TableHead></TableRow>
          </TableHeader>
          <TableBody>
            <TableEmpty v-if="!report.models.length" :colspan="4" class="text-mut">No agents have worked yet.</TableEmpty>
            <TableRow v-for="m in report.models" :key="m.model + ' ' + m.effort">
              <TableCell class="pl-4 font-medium">{{ modelEffort(m.model || 'default', m.effort) }}</TableCell><TableCell :class="num">{{ m.agents }}</TableCell><TableCell :class="num">{{ m.failed }}</TableCell><TableCell :class="num" class="pr-4">{{ m.turns }}</TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </TabsContent>
      <TabsContent value="kinds">
        <Table class="text-[12.5px]">
          <TableHeader>
            <TableRow><TableHead class="pl-4">Kind</TableHead><TableHead :class="num">Chains</TableHead><TableHead :class="num">Goal checked</TableHead><TableHead :class="num">Passed first</TableHead><TableHead :class="num">Reviewed</TableHead><TableHead :class="num">Reworked</TableHead><TableHead :class="num">Landed</TableHead><TableHead :class="num">Taken back</TableHead><TableHead :class="num" class="pr-4">On the way</TableHead></TableRow>
          </TableHeader>
          <TableBody>
            <TableEmpty v-if="!report.kinds.length" :colspan="9" class="text-mut">No agents have worked yet.</TableEmpty>
            <TableRow v-for="p in report.kinds" :key="p.kind">
              <TableCell class="pl-4 font-medium">{{ p.kind }}</TableCell><TableCell :class="num">{{ p.chains }}</TableCell><TableCell :class="num">{{ p.goalChecked }}</TableCell><TableCell :class="num">{{ p.goalFirstPass }}</TableCell><TableCell :class="num">{{ p.gated }}</TableCell><TableCell :class="num">{{ p.reworked }}</TableCell><TableCell :class="num">{{ p.landed }}</TableCell><TableCell :class="num">{{ p.reverted }}</TableCell><TableCell :class="num" class="pr-4">{{ p.landing }}</TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </TabsContent>
      <TabsContent v-for="u in usage" :key="u.value" :value="u.value">
        <Table class="text-[12.5px]">
          <TableHeader>
            <TableRow><TableHead class="pl-4">{{ u.head }}</TableHead><TableHead :class="num">Calls</TableHead><TableHead :class="num">Failed</TableHead><TableHead :class="num">Agents</TableHead><TableHead :class="num" class="pr-4">Chains</TableHead></TableRow>
          </TableHeader>
          <TableBody>
            <TableEmpty v-if="!u.rows(report).length" :colspan="5" class="text-mut">{{ u.empty }}</TableEmpty>
            <TableRow v-for="r in u.rows(report)" :key="`${r.server}/${r.name}`">
              <TableCell class="pl-4 font-medium"><span v-if="r.server" class="font-normal text-mut">{{ r.server }} / </span>{{ r.name }}</TableCell><TableCell :class="num">{{ r.calls }}</TableCell><TableCell :class="num">{{ r.failed }}</TableCell><TableCell :class="num">{{ r.agents }}</TableCell><TableCell :class="num" class="pr-4">{{ r.chains }}</TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </TabsContent>
    </PageSection>
  </Tabs>
</template>
