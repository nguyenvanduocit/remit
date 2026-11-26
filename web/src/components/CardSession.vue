<script setup lang="ts">
import { accountNamed } from '../catalog';
import { fullTime } from '../ui';
import type { Card } from '../core/generated/types.generated';
import Markdown from './Markdown.vue';
import KeyValues from './KeyValues.vue';
import CardProcesses from './CardProcesses.vue';

// How the agent runs: which Agent it is, the model, effort and mode Remit asks Claude Code for (what it runs, Claude Code
// says in ContextDetails and CardTelemetry), the account it runs on, how many turns it took, its session and directory, and the brief it was hired with. How full
// its context is, the meter above says; what fills it, ContextDetails below.
defineProps<{ card: Card }>();
</script>

<template>
  <KeyValues>
    <dt>Agent</dt><dd>{{ card.agent }}<template v-if="card.origin !== card.agent"> · {{ card.origin }}</template></dd>
    <dt>Asked for</dt><dd>{{ card.model }} · effort {{ card.effort || 'not set' }} · mode {{ card.mode }}</dd>
    <dt>Account</dt><dd>{{ card.account ? accountNamed(card.account) : '—' }}</dd>
    <dt>Turns</dt><dd>{{ card.turns }}</dd>
    <dt>Session</dt><dd class="mono">{{ card.sessionId || '—' }}</dd>
    <dt>Directory</dt><dd class="mono">{{ card.cwd }}</dd>
    <dt>Created</dt><dd>{{ fullTime(card.createdAt) }}</dd>
    <template v-if="card.error"><dt>Error</dt><dd><span class="text-red-text">{{ card.error }}</span></dd></template>
  </KeyValues>
  <CardProcesses :key="card.id" :card-id="card.id" class="mt-4" />
  <template v-if="card.brief"><h3 class="caps mt-4 mb-2">Brief</h3><Markdown type="quiet" :text="card.brief" /></template>
  <template v-if="card.deliverable"><h3 class="caps mt-4 mb-2">Deliverable</h3><Markdown type="quiet" :text="card.deliverable" /></template>
  <template v-if="card.criteria?.length">
    <h3 class="caps mt-4 mb-2">Acceptance criteria</h3>
    <ul class="list-disc pl-[18px] text-[12.5px] text-text2"><li v-for="(k, i) in card.criteria" :key="i">{{ k }}</li></ul>
  </template>
</template>
