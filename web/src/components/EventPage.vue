<script setup lang="ts">
import { useQueryRef } from '../nav';
import type { EventRecord } from '../core/generated/types.generated';
import EventFirings from './EventFirings.vue';
import PageBody from './PageBody.vue';
import PageHead from './PageHead.vue';
import EventLine from './EventLine.vue';
import Tip from './Tip.vue';
import EventRequest from './EventRequest.vue';
import { TabsTrigger } from '@/components/ui/tabs';

// The open event, in the header a chain's page has (PageHead): the signal it came through and how many subscriptions acted on
// it, its tabs under it (the one open is the address's ?tab=) and the tab's content right below. Firings is what became of it,
// subscription by subscription (EventFirings); Request is what arrived.
const props = defineProps<{ event: EventRecord }>();
const NOTE = 'what is done about an event is asked of the Assistant';
const tab = useQueryRef('tab', 'firings', ['firings', 'request'] as const);
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col">
    <PageHead :title="event.signal" mono :tab="tab" @update:tab="tab = $event as typeof tab">
      <template #standing><span class="text-[12px] text-text2" data-fired>{{ event.firings.length ? (event.firings.length === 1 ? '1 firing' : `${event.firings.length} firings`) : 'No subscription held for it' }}</span></template>
      <template #end><Tip :text="NOTE" when-clipped><span class="hidden min-w-0 truncate text-xs text-mut @2xl:block">{{ NOTE }}</span></Tip></template>
      <template #tabs>
        <TabsTrigger value="firings">Firings</TabsTrigger>
        <TabsTrigger value="request">Request</TabsTrigger>
      </template>
      <!-- What the Request tab opens with, for the tabs that are not it; it leaves the row before the tabs do. -->
      <template #aside>
        <EventLine class="hidden min-w-0 truncate text-[11.5px] text-mut @2xl:block" :record="props.event" :event="event.event" />
      </template>
    </PageHead>
    <EventFirings v-if="tab === 'firings'" :event="event" />
    <PageBody v-else><EventRequest v-if="event.event" :record="event" :event="event.event" /></PageBody>
  </div>
</template>
