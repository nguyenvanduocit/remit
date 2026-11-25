<script setup lang="ts">
import { computed } from 'vue';
import { catalog, healthBad } from '../catalog';
import { attempt } from '../act';
import { useApi } from '../core/api';
import { Button } from '@/components/ui/button';

// Under the top bar while the agents cannot run (no account can), with why and a way to check again. While any account takes
// work the bar says nothing: the Accounts page lists each account that is out. Nothing new starts while none can run; the server
// asks the accounts again on its own, and what waited starts when one answers. The provider error wraps alongside the retry button. Adding an account is the Accounts page; its sign-in is a
// login item in the inbox.
//
// Below it, while the server holds records this version cannot read (a newer one wrote them, and this one was rolled back
// to), how many it leaves out of every list: they are kept as they are, and show again once a version that reads them runs.
const api = useApi();
// The check is one real turn of an agent on each account, which the server gives two minutes.
const unreadable = computed(() => catalog.health?.unreadable ?? 0);
</script>

<template>
  <div v-if="healthBad" class="flex min-h-9 shrink-0 items-center gap-2.5 border-b border-red-line bg-red-bg2 px-4 py-1.5 text-[12.5px] text-red-hi" role="alert">
    <span class="min-w-0 [overflow-wrap:anywhere]"><b>Agents can't run.</b> {{ catalog.health?.error }} — nothing new starts until one answers.</span>
    <Button class="flex-none" variant="outline" size="sm" @click="attempt(() => api.healthCheck(), 'Checked again')">Check again</Button>
  </div>
  <div v-if="unreadable > 0" class="flex min-h-9 shrink-0 items-center border-b border-amber-line bg-amber-bg2 px-4 py-1.5 text-[12.5px] text-amber" role="status">
    <span class="min-w-0 [overflow-wrap:anywhere]"><b>{{ unreadable }} {{ unreadable === 1 ? 'record' : 'records' }} hidden</b> — written by a newer version of Remit, which this one cannot read. Kept as written; shown again once that version runs.</span>
  </div>
</template>
