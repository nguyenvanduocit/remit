<script setup lang="ts">
import { computed, ref } from 'vue';
import { useApi } from '../core/api';
import type { EventSignal, SilentSensor } from '../core/generated/types.generated';
import { actorLabel } from '../stream';
import { addressOf, KEY_MASK } from '../eventSignals';
import Ago from './Ago.vue';
import CodeBlock from './CodeBlock.vue';
import Icon from './Icon.vue';
import KeyReveal from './KeyReveal.vue';
import KeyValues from './KeyValues.vue';
import PageSection from './PageSection.vue';
import EventSignalGuide from './EventSignalGuide.vue';
import EventSignalSensors from './EventSignalSensors.vue';

// An event signal's settings, on its page on the Signals tab: what it is for, who set it and whether writers are let in, its
// door's address, the sensors that fill it (EventSignalSensors), all read-only, its key (masked until the owner asks, KeyReveal) and
// how a writer writes to it (EventSignalGuide). What happens to an event once it is in is its subscriptions', on the Subscriptions tab. Its parent keys it by
// the signal, so a key shown for one never stays when another opens. `silentSensors` is the signal line's word on which of its sensors are
// not being read.
const props = defineProps<{ eventSignal: EventSignal; silentSensors?: SilentSensor[] }>();
const api = useApi();
const address = computed(() => addressOf(location.origin, props.eventSignal.name));
// The key KeyReveal shows, for the guide's example.
const revealed = ref('');
</script>

<template>
  <PageSection title="The event signal" id="event-signal" data-event-signal>
    <p v-if="eventSignal.note" class="mb-3.5 text-text2 [overflow-wrap:anywhere]" data-note>{{ eventSignal.note }}</p>
    <KeyValues>
      <dt>{{ eventSignal.rev > 1 ? 'Changed' : 'Made' }}</dt><dd data-by>{{ actorLabel(eventSignal.by) }} · <Ago :at="eventSignal.updatedAt" /></dd>
      <dt>Writers</dt>
      <dd class="flex items-start gap-1.5" data-writers>
        <Icon class="mt-0.5 flex-none text-mut" :name="eventSignal.paused ? 'pause' : 'check'" :size="12" />
        <span>{{ eventSignal.paused ? 'Paused: no event is taken from any writer until it is resumed, and its door answers 401' : 'Let in: its door with its key, remit emit, its sensors and an agent’s emit_event' }}</span>
      </dd>
      <template v-if="eventSignal.kept">
        <dt>Events</dt>
        <dd class="flex items-start gap-1.5" data-kept>
          <Icon class="mt-0.5 flex-none text-mut" name="key" :size="12" />
          <span>Its events stay on this machine: no decider reads them, so a screen on a subscription to it asks nothing of them.</span>
        </dd>
      </template>
      <template v-if="eventSignal.builtin"><dt>Settings</dt><dd data-builtin>{{ eventSignal.edited ? 'Your version of what ships with Remit. Ask the Assistant to reset it to the shipped settings.' : 'Ships with Remit and follows it. Tell the Assistant what to change: your version is then kept.' }}</dd></template>
    </KeyValues>
    <CodeBlock class="mt-3.5" :text="address" :label="`Copy the door's address of ${eventSignal.name}`" data-address />
  </PageSection>
  <EventSignalSensors :event-signal="eventSignal" :silent-sensors="silentSensors" />
  <KeyReveal :name="eventSignal.name" :mask="KEY_MASK" :read="() => api.eventSignalKey(eventSignal.id)" @revealed="revealed = $event" />
  <EventSignalGuide class="@[1100px]:col-span-2" :name="eventSignal.name" :address="address" :revealed="revealed" />
</template>
