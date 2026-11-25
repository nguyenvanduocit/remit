<script setup lang="ts">
import PageSection from './PageSection.vue';
import { computed } from 'vue';
import { useApi } from '../core/api';
import { useRemote } from '../useRemote';
import { actorName } from '../stream';
import { fullTime, plural } from '../ui';
import ActorTag from './ActorTag.vue';
import Ago from './Ago.vue';
import ListState from './ListState.vue';
import Fold from './Fold.vue';
import Tip from './Tip.vue';
import type { Actor, Change, ChangeKind } from '../core/generated/types.generated';

// What was written to a record, newest first: who wrote it, when, and each field that differs as it was and as it became, one
// line each that opens to the whole of both values. The server keeps it (changesGet); it is read when the record opens and
// again when the record moves, since a write is what moves it. A write that waited for the owner says who let it through.
const props = defineProps<{ kind: ChangeKind; name: string; rev: number }>();
const api = useApi();
const history = useRemote(
  () => ({ kind: props.kind, name: props.name, rev: props.rev }),
  (asked) => api.changesGet(asked.kind, asked.name),
);
const changes = computed(() => history.data.value ?? []);
// An agent shows as one: its card is in a chain the owner may not have open.
const anAgent = () => 'An agent';
const who = (by: Actor) => actorName(by, anAgent);
const what = (c: Change) => (c.action === 'created' ? 'created it' : c.action === 'deleted' ? 'deleted it' : `changed ${plural(c.fields.length, 'field')}`);
// A field's whole value, opened under its line: the JSON the server wrote, laid out over lines so a list or an object reads.
const shown = (value: string) => {
  if (!value) return '—';
  try {
    return JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    return value;
  }
};
const approval = (by: Actor) => (by.kind === 'owner' ? 'let through by you' : by.kind === 'assistant' ? 'let through by the Assistant, for you' : `let through by ${who(by)}`);
</script>

<template>
  <PageSection title="History" :count="changes.length">
    <ListState what="the history" :held="!!history.data.value" :error="history.error.value" :empty="!changes.length" :retry="history.retry">
    <template #empty>No changes recorded yet: each write to it shows here, with who wrote it.</template>
    <ol class="m-0 list-none p-0">
      <li v-for="c in changes" :key="c.id" data-change class="change">
        <div class="flex min-w-0 items-center gap-1.5">
          <Tip v-if="c.by.who" :text="c.by.who"><ActorTag :by="c.by" :name-of="anAgent" /></Tip><ActorTag v-else :by="c.by" :name-of="anAgent" />
          <span class="min-w-0 text-text">{{ what(c) }}<span v-if="c.approvedBy" class="ok"> · {{ approval(c.approvedBy) }}</span></span>
          <Tip :text="fullTime(c.at)"><time class="mono ml-auto flex-none text-faint" :datetime="c.at"><Ago :at="c.at" /></time></Tip>
        </div>
        <ul v-if="c.action === 'changed'" class="fields">
          <li v-for="f in c.fields" :key="f.name">
            <Fold>
              <template #summary><code class="mono name">{{ f.name }}</code><span class="was">{{ f.before || '—' }}</span><span class="arrow">→</span><span class="now">{{ f.after || '—' }}</span></template>
              <dl class="full">
                <dt>was</dt><dd>{{ shown(f.before) }}</dd>
                <dt>now</dt><dd>{{ shown(f.after) }}</dd>
              </dl>
            </Fold>
          </li>
        </ul>
      </li>
    </ol>
    </ListState>
  </PageSection>
</template>

<style scoped>
.change {
  padding: 8px 0;
  border-bottom: 1px dashed var(--rule);
  font-size: 12.5px;
}
.ok {
  color: var(--mut);
}
/* Each field is one line, the name then both values cut to fit; opened, it shows both whole, one above the other. */
.fields {
  margin: 4px 0 0;
  padding: 0;
  list-style: none;
  font-size: 11.5px;
}
.fields :deep(summary) {
  min-width: 0;
  padding: 1px 4px;
  margin: 0 -4px;
}
.fields :deep(summary):hover {
  background: var(--panel3);
}
.name {
  flex: none;
  min-width: 88px;
  color: var(--text2);
}
.was,
.now {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  font-family: var(--mono);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.was {
  color: var(--mut);
}
.now {
  color: var(--text2);
}
.arrow {
  flex: none;
  color: var(--faint);
}
.full {
  display: grid;
  grid-template-columns: max-content minmax(0, 1fr);
  gap: 4px 10px;
  margin: 4px 0 8px;
  padding: 8px 10px;
  border-radius: 6px;
  background: var(--bg2);
}
.full dt {
  color: var(--mut);
}
.full dd {
  margin: 0;
  max-height: 320px;
  overflow: auto;
  font-family: var(--mono);
  color: var(--text2);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
</style>
