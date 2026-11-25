<script setup lang="ts">
import { computed } from 'vue';
import { catalog } from '../catalog';
import type { EventSignal, Sensor, SilentSensor } from '../core/generated/types.generated';
import { everyWords, kindLook, settingRows } from '../sensorWords';
import CodeBlock from './CodeBlock.vue';
import EmptyState from './EmptyState.vue';
import Fold from './Fold.vue';
import Icon from './Icon.vue';
import KeyValues from './KeyValues.vue';
import PageSection from './PageSection.vue';

// The sensors that fill one event signal (ADR-0047, ADR-0049), read-only, whatever their kind: a poll reads a source's connection on
// a period and writes what is new as an event of the signal; a plugin's kind shows the settings it was given. A query that starts
// with `#!` is a script, shown folded. A sensor is made and changed by telling
// the Assistant, so nothing here edits one. `silentSensors` are the signal line's word on which of its sensors are not being read, and why.
const props = defineProps<{ eventSignal: EventSignal; silentSensors?: SilentSensor[] }>();
const sensors = computed(() => catalog.sensors.filter((s) => s.signal === `event:${props.eventSignal.name}`));
// The source a sensor reads, by the name the Tools tab gives it; one Remit no longer has reads as its id.
const sourceOf = (s: Sensor) => catalog.sources.find((x) => x.id === s.source);
const isScript = (s: Sensor) => !!s.query?.startsWith('#!');
const isPlugin = (s: Sensor) => s.kind.startsWith('plugin:');
const silentWhy = (s: Sensor) => props.silentSensors?.find((x) => x.sensorId === s.id)?.why;
</script>

<template>
  <PageSection title="Sensors" id="event-signal-sensors" :count="sensors.length || undefined" data-sensors>
    <ul v-if="sensors.length" class="m-0 flex list-none flex-col gap-4 p-0">
      <li v-for="s in sensors" :key="s.id" class="min-w-0 border-t border-line pt-4 first:border-t-0 first:pt-0" data-sensor :data-sensor-name="s.name">
        <h3 class="mb-1 text-[13px] font-semibold text-text [overflow-wrap:anywhere]" data-sensor-title>{{ s.name }}</h3>
        <p v-if="s.note" class="mb-2.5 text-text2 [overflow-wrap:anywhere]" data-sensor-note>{{ s.note }}</p>
        <KeyValues>
          <template v-if="isPlugin(s)">
            <dt>Kind</dt>
            <dd data-sensor-kind><span class="mono">{{ kindLook(s.kind).noun }}</span><template v-if="kindLook(s.kind).plugin"> from the plugin <span class="mono">{{ kindLook(s.kind).plugin }}</span></template></dd>
            <template v-for="[k, v] in settingRows(s.settings)" :key="k"><dt>{{ k }}</dt><dd class="mono [overflow-wrap:anywhere]" data-sensor-setting>{{ v }}</dd></template>
          </template>
          <template v-if="s.source">
            <dt>Source</dt>
            <dd data-sensor-source><RouterLink v-if="sourceOf(s)" class="text-blue-text hover:underline" :to="{ name: 'tools', params: { sourceId: s.source } }">{{ sourceOf(s)?.label }}</RouterLink><template v-else>{{ s.source }}</template></dd>
          </template>
          <template v-if="s.connection"><dt>Connection</dt><dd class="mono" data-sensor-connection>{{ s.connection }}</dd></template>
          <template v-if="isScript(s) || s.script">
            <dt>Script</dt>
            <dd class="min-w-0" data-sensor-script>
              <Fold>
                <template #summary><span class="text-xs text-blue-text">Show the script</span></template>
                <CodeBlock class="mt-1.5" :text="s.script?.code ?? s.query ?? ''" label="the script" wrap />
              </Fold>
            </dd>
          </template>
          <template v-else-if="s.query"><dt>Query</dt><dd class="mono" data-sensor-query>{{ s.query }}</dd></template>
          <template v-if="everyWords(s)"><dt>Reads</dt><dd data-sensor-every>{{ everyWords(s) }}</dd></template>
          <dt>State</dt>
          <dd class="flex items-start gap-1.5" :class="!s.paused && silentWhy(s) ? 'text-warning' : ''" data-sensor-state>
            <Icon class="mt-0.5 flex-none" :class="!s.paused && silentWhy(s) ? '' : 'text-mut'" :name="s.paused ? 'pause' : silentWhy(s) ? 'alert' : 'repeat'" :size="12" />
            <span class="min-w-0 [overflow-wrap:anywhere]">{{ s.paused ? 'Paused: reads nothing until it is resumed' : silentWhy(s) ? `Silent: ${silentWhy(s)}. Nothing comes in from it until it answers again.` : everyWords(s) ? 'Reading on its period' : 'Sensing' }}</span>
          </dd>
        </KeyValues>
      </li>
    </ul>
    <EmptyState v-else data-sensors-none>No sensor: only its writers fill it (its door, remit emit, an agent). Ask the Assistant to add a sensor that reads a source into it, such as a repository’s GitHub issues or Datadog monitors.</EmptyState>
  </PageSection>
</template>
