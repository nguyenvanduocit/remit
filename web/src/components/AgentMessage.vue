<script setup lang="ts">
// A message into or out of an agent, as two lines in the stream of the agent that read it (`from`) or sent it (`to`): who,
// which way, what kind and when over what it is about; the message itself is under them until it is opened. It is a turning point in
// the work, so it stands out from the steps around it. `title` names the message in place of the way and who (a person's hand-in).
import Icon from './Icon.vue';
import { hm } from '../ui';
import Fold from './Fold.vue';
import { Badge } from '@/components/ui/badge';

defineProps<{ dir: 'from' | 'to'; who: string; label: string; kind?: string; at?: string; note?: string; title?: string }>();
</script>

<template>
  <Fold class="agent-msg" :data-dir="dir">
    <template #summary>
      <Icon class="mark" :name="dir === 'from' ? 'arrow-left' : 'arrow-right'" :size="13" />
      <span class="head">
        <span v-if="title" class="who">{{ title }}</span>
        <span v-else class="who"><span class="sr-only">{{ dir === 'from' ? 'From' : 'To' }} </span>{{ who }}</span>
        <Badge v-if="kind" variant="assistant" data-kind>{{ kind }}</Badge>
        <span v-if="note" class="note">{{ note }}</span>
      </span>
      <time v-if="at" class="time">{{ hm(at) }}</time>
      <span class="label">{{ label }}</span>
    </template>
    <slot />
  </Fold>
</template>

<style scoped>
@reference "../theme.css";

/* A surface of its own, in the violet an agent's name wears wherever it speaks (the state shows as the surface, never as a
   ring on its border). */
.agent-msg {
  margin: 8px 0;
  padding: 6px 10px;
  border-radius: 8px;
  background: var(--violet-wash);
  font-size: 12px;
}
/* The arrow, then who it is with (and the time at the right) over what it is about, in the column under the name. */
.agent-msg :deep(summary) {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 2px 8px;
}
.head {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 8px;
}
.time {
  font-size: 11px;
  color: var(--mut);
}
.label {
  grid-column: 2 / -1;
  min-width: 0;
  /* what the message is about is what the owner scans for: two lines before it is cut */
  display: -webkit-box;
  overflow: hidden;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  color: var(--text2);
  overflow-wrap: anywhere;
}
/* Opened, the message is under the line, so what it is about is no longer needed there. */
.agent-msg[open] .label {
  display: none;
}
.mark {
  color: var(--violet);
}
.who {
  min-width: 0;
  overflow: hidden;
  font-weight: 600;
  color: var(--violet-text);
  text-overflow: ellipsis;
  white-space: nowrap;
}
/* A message the Chair put into the turn that was running: read there, not after it. */
.note {
  flex: none;
  font-size: 11px;
  color: var(--blue);
}
.agent-msg > :deep(:not(summary)) {
  margin-top: 6px;
}
</style>
