<script setup lang="ts">
import { computed } from 'vue';
import { bodySize, VIA } from '../events';
import { clockAt } from '../ui';
import { pathOf } from '../eventSignals';
import type { EventRecord, InboundEvent } from '../core/generated/types.generated';
import Ago from './Ago.vue';
import Tip from './Tip.vue';

// What came in, in one line, the same wherever an event says it: the way it came (its word, and the path a post went to), how
// big it was and when it came. An event alone (a list's, whose arrival is not read) says the signal and when. Its parent sizes
// and cuts it; cut off, the line shows whole on hover (with the time of day, which does not age). The parent's classes are the
// line's, not the hint's.
defineOptions({ inheritAttrs: false });
const props = defineProps<{ record: EventRecord; event?: InboundEvent }>();
const via = computed(() => (props.event ? VIA[props.event.via] : undefined));
const where = computed(() => (via.value?.posts || !props.event ? pathOf(props.record.signal, props.event?.query) : `event:${props.record.signal}`));
const whole = computed(() => {
  const head = via.value ? `${via.value.word} ${where.value} · ${bodySize(props.event!)}` : where.value;
  return `${head} · ${clockAt(props.record.at)}`;
});
</script>

<template>
  <Tip :text="whole" when-clipped>
    <span v-bind="$attrs" :data-via="event?.via" data-line><template v-if="via"><span class="text-text">{{ via.word }}</span>{{ ' ' }}</template><span class="mono break-all">{{ where }}</span><template v-if="event"> · {{ bodySize(event) }}</template> · <Ago :at="record.at" /> · {{ clockAt(record.at) }}</span>
  </Tip>
</template>
