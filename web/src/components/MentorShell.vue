<script setup lang="ts">
import { computed, watch } from 'vue';
import { useRoute } from 'vue-router';
import { catalog, chainRecords } from '../catalog';
import { columnsOf } from '../board';
import { said } from '../banner';
import { opened, openChain } from '../openChain';
import { oneOf } from '../core/client';
import { useModel } from '../replica';
import { session } from '../session';
import { signOut } from '../wayIn';
import { assistantColumn } from '../panels';
import type { Who } from '../core/generated/types.generated';
import BoardColumn from './BoardColumn.vue';
import ConnectionBanner from './ConnectionBanner.vue';
import EmptyState from './EmptyState.vue';
import Icon from './Icon.vue';
import MentorHelp from './MentorHelp.vue';
import Spinner from './Spinner.vue';
import { Button } from '@/components/ui/button';
import { Toaster } from '@/components/ui/sonner';
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable';
import { TooltipProvider } from '@/components/ui/tooltip';

// A mentor's app (ADR-0089): the owner's board, as the server lays it out for them (Asked, Helping, Done: the chains of
// their helps, in the owner's columns and cards), and beside it, in the column the owner's Assistant takes, the agent they
// help in the chain a card opens (/chains/<id>), in the owner's drawer. What they see and may do is the server's to say.
const route = useRoute();
const whoRows = useModel<Who>({ model: 'who' });
const who = computed(() => oneOf(whoRows));
const name = computed(() => (who.value?.kind === 'mentor' ? who.value.name : ''));
const columns = computed(() => columnsOf(catalog.board, chainRecords.value, [], []).filter((c) => c.key !== 'archived'));
const chainId = computed(() => (route.name === 'chain' && typeof route.params.id === 'string' ? route.params.id : undefined));
watch(
  chainId,
  (id) => {
    if (id && opened.view?.chain.chain.id !== id && opened.opening !== id) openChain(id);
  },
  { immediate: true },
);
const view = computed(() => (opened.view && opened.view.chain.chain.id === chainId.value ? opened.view : null));
const offline = computed(() => session.link.phase === 'offline');
</script>

<template>
  <TooltipProvider :delay-duration="300">
    <div v-if="offline" class="flex h-full flex-col items-center justify-center gap-3 px-6 text-[13px] text-mut" role="status" data-phase="offline">
      <p class="flex max-w-md items-center gap-2"><Spinner :size="12" />{{ session.banner && said(session.banner) }}</p>
    </div>
    <div v-else class="flex h-full flex-col" data-mentor>
      <header class="flex h-11 flex-none items-center gap-2 border-b border-line px-4">
        <Icon class="text-blue-mark" name="link" :size="16" /><b class="text-[14px]">Remit</b>
        <span v-if="name" class="text-mut" data-mentor-name>· {{ name }}, mentor</span>
        <Button class="ml-auto" variant="ghost" size="sm" data-sign-out @click="signOut()">Sign out</Button>
      </header>
      <ConnectionBanner />
      <ResizablePanelGroup direction="horizontal" auto-save-id="mentor" class="min-h-0 flex-1">
        <ResizablePanel id="mentor-board" :order="1" class="flex" :min-size="35">
          <main id="main" class="flex min-h-0 min-w-0 flex-1 flex-col" :aria-busy="catalog.loaded ? undefined : true">
            <h1 class="sr-only">Your helps</h1>
            <div v-if="catalog.loaded" class="grid min-h-0 flex-1 auto-cols-[minmax(300px,1fr)] grid-flow-col gap-2.5 overflow-x-auto px-4 pt-2.5 pb-3.5">
              <BoardColumn v-for="col in columns" :key="col.key" class="rounded-lg bg-bg2 p-2" :column="col" :open-id="chainId" />
            </div>
            <EmptyState v-else class="flex items-center gap-2 p-6"><Spinner :size="12" />Loading…</EmptyState>
          </main>
        </ResizablePanel>
        <template v-if="chainId">
          <ResizableHandle aria-label="Resize the agent you help" />
          <ResizablePanel id="mentor-help" :order="2" class="flex" v-bind="assistantColumn" :default-size="560">
            <MentorHelp v-if="view && who?.kind === 'mentor'" :view="view" :mentor-id="who.mentorId" />
            <EmptyState v-else class="flex items-center gap-2 p-6" role="status"><Spinner :size="12" />Opening…</EmptyState>
          </ResizablePanel>
        </template>
      </ResizablePanelGroup>
    </div>
    <Toaster :duration="2800" position="bottom-center" />
  </TooltipProvider>
</template>
