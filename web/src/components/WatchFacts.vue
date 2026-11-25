<script setup lang="ts">
import { computed } from 'vue';
import { catalog } from '../catalog';
import type { SubscriptionLine } from '../core/generated/types.generated';
import CodeBlock from './CodeBlock.vue';
import Fold from './Fold.vue';
import SignalLink from './SignalLink.vue';

// What a watch reads and when it wakes the Chair, as rows of a KeyValues list: the signal, the condition over a reading (none:
// every new reading) and, when the Chair made a script sensor to read what no source gives it, that sensor: the one of the
// replica that writes this key for this chain. Both the Subscriptions tab and the chain's page show it.
const props = defineProps<{ line: SubscriptionLine }>();
const sensor = computed(() => catalog.sensors.find((s) => s.signal === props.line.key && s.chain === props.line.chainId));
</script>

<template>
  <dt>Signal</dt><dd data-key><SignalLink :signal-key="line.key" :source="line.source" /></dd>
  <dt>Wakes the Chair</dt><dd data-condition><span v-if="line.condition" class="mono">{{ line.condition }}</span><template v-else>at every new reading</template></dd>
  <template v-if="sensor?.script">
    <dt>Reads with</dt>
    <dd class="flex min-w-0 flex-col gap-1.5" data-sensor>
      <span>A {{ sensor.script.language }} script through {{ sensor.source }}/{{ sensor.connection }}, run every {{ sensor.everyMinutes }} min<template v-if="sensor.paused"> (paused)</template></span>
      <Fold>
        <template #summary><span class="text-xs text-blue-text">Show the script</span></template>
        <CodeBlock class="mt-1.5" :text="sensor.script.code" label="the script" wrap />
      </Fold>
    </dd>
  </template>
</template>
