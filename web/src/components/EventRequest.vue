<script setup lang="ts">
import { computed } from 'vue';
import { bodyShown, VIA } from '../events';
import type { EventRecord, InboundEvent } from '../core/generated/types.generated';
import CodeBlock from './CodeBlock.vue';
import EmptyState from './EmptyState.vue';
import Icon from './Icon.vue';
import KeyValues from './KeyValues.vue';
import EventLine from './EventLine.vue';

// What arrived, as its writer sent it: the way it came (its door, the CLI, a sensor, an agent), where and how big, when it came
// (EventLine), every header it set but the credentials, and the body (indented when it is JSON,
// the start of it when the server cut a long one). An event a sensor read or an agent wrote has no headers: it was not posted.
const props = defineProps<{ record: EventRecord; event: InboundEvent }>();
const via = computed(() => VIA[props.event.via]);
const headers = computed(() => Object.entries(props.event.header ?? {}));
const body = computed(() => bodyShown(props.event));
</script>

<template>
  <div class="flex flex-col gap-4" data-request>
    <KeyValues>
      <dt>Came in</dt>
      <dd class="flex items-center gap-1.5" data-via><Icon class="flex-none text-mut" :name="via.icon" :size="12" />{{ via.says }}</dd>
      <dt>Event</dt>
      <dd><EventLine :record="record" :event="event" /></dd>
    </KeyValues>
    <KeyValues v-if="headers.length">
      <template v-for="[name, value] in headers" :key="name"><dt class="mono break-all">{{ name }}</dt><dd class="mono">{{ value }}</dd></template>
    </KeyValues>
    <EmptyState v-else-if="via.posts" class="text-[12px]">The writer set no headers.</EmptyState>
    <CodeBlock :text="body" label="Copy the body" wrap data-body />
    <p v-if="event.bodyCut" class="text-[11.5px] text-mut" data-cut>The body is cut: Remit keeps the first 64 KiB of a longer one.</p>
  </div>
</template>
