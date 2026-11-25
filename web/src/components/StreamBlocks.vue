<script setup lang="ts">
import Icon from './Icon.vue';
import { computed, ref } from 'vue';
import { useStream } from '../streamContext';
import { imageUrl } from '../images';
import { actorName, agentSender, dataOf, foldedRuns, isStep, mentorSaid, mentorText, noteOf, ownerSent, permissionLabel, stepLook, TOOL_ICON as ICON, toolCall, toolOf, type Block } from '../stream';
import { durSec, hm, plural, short } from '../ui';
import Markdown from './Markdown.vue';
import type { EventRead } from '../core/generated/types.generated';
import Tip from './Tip.vue';
import AgentMessage from './AgentMessage.vue';
import BlockView from './BlockView.vue';
import ReplyTeach from './ReplyTeach.vue';
import TurnLine from './TurnLine.vue';
import MentorMark from './MentorMark.vue';
import Callout from './Callout.vue';
import CallCard from './CallCard.vue';
import CodeBlock from './CodeBlock.vue';
import CopyButton from './CopyButton.vue';
import Fold from './Fold.vue';
import ReceiptLine from './ReceiptLine.vue';
import ShowMore from './ShowMore.vue';
import Sheen from './Sheen.vue';
import StandingMark from './StandingMark.vue';
import TaskRow from './TaskRow.vue';
import VerdictEntry from './VerdictEntry.vue';
import { Badge } from '@/components/ui/badge';

// One card's stream, folded. `live` marks the card as still working, so its newest
// thought stays open and its newest message carries a caret.
const props = defineProps<{ blocks: Block[]; live: boolean; compact?: boolean }>();

const open = ref<Record<string, boolean>>({}); // runs of steps, turns' steps, tool groups and tool rows the owner opened
// What the owner sent is a chat bubble (ownerSent); bootstrap prompts and lifecycle notes fold into one line. The owner's
// message to the Assistant carries where he was as he sent it, in the server's words.
const stream = useStream();
const nameOf = (id?: string) => (id ? stream.nameOf(id) : 'System');
// Who a message into this agent came from: another agent, its own background work, or the System.
const senderName = (e: EventRead) => (agentSender(e) === e.event.cardId ? 'Its background work' : nameOf(agentSender(e)));
const isLast = (i: number) => i === props.blocks.length - 1;
// A long run of steps a message closed shows as one bar until the owner opens it; opened, it stays open.
const runs = computed(() => foldedRuns(props.blocks));
const foldOf = (i: number) => runs.value.find((r) => i >= r.start && i < r.end && !open.value[r.key]);
const groupOpen = (b: Extract<Block, { type: 'tools' }>, i: number) => open.value[b.key] ?? (b.running && props.live && isLast(i));
function fmt(v: unknown): string {
  return typeof v === 'string' ? v : JSON.stringify(v, null, 2);
}
// The stream in turns: from the message that opened one (a prompt) to the line that ends it, one frame, so a turn reads as
// one piece of work; what stands outside any turn (a job the server ran, a task that ended between turns) stands on its
// own. A frame whose every row is folded away is not drawn.
interface Turn {
  key: string;
  turn: boolean;
  items: { b: Block; i: number }[];
}
const turns = computed(() => {
  const out: Turn[] = [];
  let cur: Turn | null = null;
  props.blocks.forEach((b, i) => {
    if (b.type === 'prompt') {
      cur = { key: 't' + b.key, turn: true, items: [] };
      out.push(cur);
    } else if (!cur) {
      cur = { key: 'o' + b.key, turn: false, items: [] };
      out.push(cur);
    }
    cur.items.push({ b, i });
    if (b.type === 'turnEnd') cur = null;
  });
  return out;
});
// A turn that ended with the row the server summed it up in (TurnLine) folds its steps into that row: the turn reads as what
// was said and one line of what was done. Each step's index maps to the key of the turn end that stands for it; opened
// ('s' + that key), the steps unfold under that row, in the order they happened, where the owner clicked.
const turnOf = computed(() => {
  const m = new Map<number, string>();
  for (const t of turns.value) {
    const end = t.items[t.items.length - 1]?.b;
    if (end?.type === 'turnEnd' && end.turn) for (const { b, i } of t.items) if (isStep(b)) m.set(i, end.key);
  }
  return m;
});
const stepsOf = computed(() => {
  const m = new Map<string, Block[]>();
  for (const [i, k] of turnOf.value) m.set(k, [...(m.get(k) ?? []), props.blocks[i]]);
  return m;
});
const inTurnFold = (i: number) => turnOf.value.has(i);
const drawn = (t: Turn) => t.items.some(({ i }) => !inTurnFold(i) && (!foldOf(i) || foldOf(i)!.start === i));
// What stands outside a turn is drawn as it is, with nothing around it: an ask the stream ends on stays the last thing in it.
const Bare = (_: unknown, { slots }: { slots: { default?: () => unknown } }) => slots.default?.();
const groupsOf = (events: EventRead[]) => [...new Set(events.map((e) => toolOf(e).group))].slice(0, 3);
const groupMs = (events: EventRead[]) => events.reduce((s, e) => s + Math.max(0, toolCall(e).ms), 0);
</script>

<template>
  <template v-for="t in turns" :key="t.key">
  <component :is="t.turn ? 'div' : Bare" v-if="drawn(t)" :class="t.turn ? 'turn' : undefined" :data-turn="t.turn ? '' : undefined">
  <template v-for="{ b, i } in t.items" :key="b.key">
    <template v-if="inTurnFold(i)" />
    <ShowMore v-else-if="foldOf(i)?.start === i" class="my-1.5" @click="open[foldOf(i)!.key] = true">Show {{ plural(foldOf(i)!.end - i, 'step') }}</ShowMore>
    <template v-else-if="foldOf(i)" />

    <div v-else-if="b.type === 'prompt' && ownerSent(b.event)" class="mine" :data-by="b.event.event.by!.kind"><div class="meta">{{ actorName(b.event.event.by!, stream.nameOf) }} · {{ hm(b.event.event.createdAt) }}</div>
      <div v-if="b.event.event.text.trim()" class="bubble" :class="{ queued: b.event.event.status === 'queued', dropped: b.event.event.status === 'dropped', assistant: b.event.event.by!.kind === 'assistant' }">{{ b.event.event.text }}</div>
      <div v-if="dataOf(b.event).images?.length" class="shots" data-images><img v-for="(i, n) in dataOf(b.event).images" :key="i.id" :src="imageUrl(i.id)" :alt="`Image ${n + 1} of your message`" class="shot" /></div>
      <ReceiptLine class="mt-[3px]" :event="b.event" /></div>

    <div v-else-if="b.type === 'prompt' && mentorSaid(b.event)" class="said" data-mentor-said>
      <div class="said-h"><MentorMark :size="16" label="Mentor" /><span class="said-who">{{ actorName(b.event.event.by!, stream.nameOf) }}</span><time class="said-time" :datetime="b.event.event.createdAt">{{ hm(b.event.event.createdAt) }}</time></div>
      <div class="said-text">{{ mentorText(b.event) }}</div>
    </div>

    <template v-else-if="b.type === 'prompt'">
      <AgentMessage dir="from" :who="senderName(b.event)" :label="short(b.event.event.title || b.event.event.text, compact ? 90 : 140)" :at="b.event.event.createdAt" :note="b.event.event.status === 'steered' ? 'read in this turn' : undefined">
        <Markdown class="prompt-text" :text="b.event.event.text" />
      </AgentMessage>
    </template>

    <AgentMessage v-else-if="b.type === 'sent'" dir="to" :who="b.to ? nameOf(b.to) : 'Chair'" :kind="b.kind" :label="short(b.label, compact ? 90 : 140)" :at="b.event.event.createdAt" :data-sent="b.kind">
      <Markdown class="prompt-text" :text="b.text" />
    </AgentMessage>

    <Fold v-else-if="b.type === 'thought'" class="thought" :open="live && isLast(i)">
      <template #summary><span class="think"><Icon name="spark" :size="11" />Thinking</span><Tip v-if="!(live && isLast(i))" :text="b.event.event.text" when-clipped><span class="min-w-0 truncate text-[11px] text-faint">{{ short(b.event.event.text, 100) }}</span></Tip></template>
      <Markdown type="quiet" class="mt-1" :text="b.event.event.text" :writing="live && isLast(i)" />
    </Fold>

    <div v-else-if="b.type === 'message'" class="message">
      <Markdown type="body" :text="b.event.event.text" :chain-tab="stream.chainTab" :writing="live && isLast(i)" />
      <ReplyTeach v-if="!(live && isLast(i))" :event="b.event"><template #actions><CopyButton :text="b.event.event.text" label="Copy message" icon-only /></template></ReplyTeach>
    </div>

    <BlockView v-else-if="b.type === 'block'" class="my-2" :event="b.event" :block="b.block" />

    <div v-else-if="b.type === 'tools'" class="my-1.5">
      <button type="button" class="tools" :aria-expanded="groupOpen(b, i)" @click="open[b.key] = !groupOpen(b, i)">
        <span class="inline-flex"><span v-for="g in groupsOf(b.events)" :key="g" class="kind"><Icon :name="ICON[g]" :size="10" /></span></span>
        <span class="flex-none whitespace-nowrap"><Sheen :text="b.running && live && isLast(i) ? 'Using tools…' : 'Used ' + plural(b.events.length, 'tool')" :on="b.running && live && isLast(i)" /></span>
        <span v-if="b.failed" class="inline-flex flex-none items-center gap-1 font-medium whitespace-nowrap text-red-text" data-failed><Icon name="alert" :size="11" />{{ b.failed }} failed</span>
        <span class="min-w-0 flex-1 truncate text-faint">· {{ b.summary }}</span>
        <span class="flex-none whitespace-nowrap text-faint">{{ durSec(groupMs(b.events)) }}</span>
      </button>
      <div v-if="groupOpen(b, i)" class="mt-1 mb-1.5 flex flex-col gap-[5px]">
        <CallCard
          v-for="e in b.events"
          :key="e.event.id"
          v-model:open="open['t' + e.event.id]"
          :icon="ICON[toolOf(e).group]"
          :name="toolCall(e).name"
          :summary="toolCall(e).summary"
          :look="stepLook(toolOf(e).status)"
          :time="durSec(toolCall(e).ms)"
          :failed="toolOf(e).status === 'failed'"
        >
          <template v-if="dataOf(e).input !== undefined"><div class="caps my-[3px]">Parameters</div><CodeBlock class="mb-1.5 max-h-[220px] overflow-auto" :text="fmt(dataOf(e).input)" label="Copy parameters" wrap /></template>
          <template v-if="dataOf(e).output"><div class="caps my-[3px]">Result</div><CodeBlock class="mb-1.5 max-h-[220px] overflow-auto" :text="dataOf(e).output ?? ''" label="Copy result" wrap /></template>
          <div v-if="!dataOf(e).output" class="text-faint">{{ toolCall(e).running ? 'Waiting for the result…' : dataOf(e).input === undefined ? 'No details.' : 'No result text recorded.' }}</div>
        </CallCard>
      </div>
    </div>

    <TurnLine v-else-if="b.type === 'turnEnd'" v-model:open="open['s' + b.key]" class="mt-1.5 mb-0.5" :event="b.event" :ms="b.ms" :turn="b.turn" :steps="stepsOf.get(b.key)?.length ?? 0">
      <StreamBlocks :blocks="stepsOf.get(b.key) ?? []" :live="false" :compact="compact" />
    </TurnLine>

    <TaskRow v-else-if="b.type === 'work' && b.event.event.kind === 'task'" :event="b.event" />

    <div v-else-if="b.type === 'work'" class="my-1.5 flex min-w-0 items-center gap-1.5 text-xs text-text2" :data-work="b.event.event.id">
      <StandingMark v-bind="stepLook(b.event.event.status)" />
      <Tip :text="b.event.event.title" when-clipped><b class="max-w-[60%] shrink-0 truncate font-medium">{{ b.event.event.title }}</b></Tip>
      <span v-if="stepLook(b.event.event.status).working" class="text-faint">running…</span>
      <template v-else>
        <Tip :text="b.event.event.text" when-clipped><span class="min-w-0 truncate text-faint">· {{ b.event.event.text }}</span></Tip>
        <span class="ml-auto flex-none text-[11px] text-faint">{{ durSec(Date.parse(b.event.event.updatedAt) - Date.parse(b.event.event.createdAt)) }}</span>
      </template>
    </div>

    <Callout v-else-if="b.type === 'system' && b.event.event.status === 'error'" tone="error" class="my-2"><template #title>Error</template><div class="text-xs break-words whitespace-pre-wrap">{{ b.event.event.text }}</div></Callout>

    <div v-else-if="b.type === 'system' && b.event.event.status === 'new_session'" class="session-break" data-session-break>
      <div class="flex items-center gap-2.5">
        <span class="rule" />
        <Badge variant="assistant"><Icon name="spark" />{{ b.event.event.title }}</Badge>
        <span class="rule" />
      </div>
      <p class="text">{{ b.event.event.text }}</p>
    </div>

    <div v-else-if="b.type === 'decision' || b.type === 'asked'" class="decision" :data-decision="b.type === 'decision' ? '' : undefined" :data-asked="b.type === 'asked' ? '' : undefined">
      <button type="button" class="decision-h" :aria-expanded="!!open['d' + b.event.event.id]" @click="open['d' + b.event.event.id] = !open['d' + b.event.event.id]">
        <StandingMark v-bind="noteOf(b.event)" />
        <span class="flex-none font-medium text-mut">{{ permissionLabel(b.event) }}</span>
        <template v-if="!open['d' + b.event.event.id]">
          <span v-if="b.event.event.text" class="min-w-0 flex-1 truncate text-faint">{{ b.event.event.text }}</span>
          <span v-else class="mono min-w-0 flex-1 truncate text-faint">{{ b.request }}</span>
        </template>
      </button>
      <div v-if="open['d' + b.event.event.id]" class="why">
        <p v-if="b.event.event.text" class="mb-1">{{ b.event.event.text }}</p>
        <CodeBlock class="max-h-[220px] overflow-auto" :text="b.event.event.title" label="Copy request" wrap />
      </div>
    </div>

    <div v-else-if="b.type === 'instructions'" class="decision" data-instructions>
      <button type="button" class="decision-h" :aria-expanded="!!open[b.key]" @click="open[b.key] = !open[b.key]">
        <StandingMark icon="book" tone="quiet" />
        <span class="flex-none text-mut">Read instructions</span>
        <span class="mono min-w-0 flex-1 truncate text-faint">{{ b.events.map((e) => e.event.title).join(', ') }}</span>
      </button>
      <ul v-if="open[b.key]" class="why">
        <li v-for="e in b.events" :key="e.event.id"><span class="mono text-text2">{{ e.event.title }}</span> · {{ e.event.text }}</li>
      </ul>
    </div>

    <div v-else-if="b.type === 'system'" class="note"><StandingMark v-bind="noteOf(b.event)" /><span>{{ b.event.event.text }}</span></div>

    <div v-else-if="b.type === 'ask'" class="note ask" data-ask-record><StandingMark tone="needs" icon="clock" /><span>Needs you: {{ b.item.title }}</span></div>

    <VerdictEntry v-else-if="b.type === 'verdict'" :judge="b.judge" class="my-3" />
  </template>
  </component>
  </template>
</template>

<style scoped>
@reference "../theme.css";

/* One turn: its rows under the message that opened it, to the line that ends it; the space between turns sets them apart. */
.turn {
  margin: 8px 0;
}
.turn > :deep(.agent-msg:first-child),
.turn > :deep(.mine:first-child) {
  margin-top: 0;
}
/* What the owner typed: a bubble across the column, its name and receipt on the right. */
.mine {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  margin: 8px 0;
}
.meta {
  margin-bottom: 3px;
  font-size: 10.5px;
  color: var(--faint);
}
.bubble {
  align-self: stretch;
  padding: 7px 11px;
  @apply owner-bubble;
  color: var(--text);
  font-size: var(--size-read);
  line-height: var(--leading-read);
  white-space: pre-wrap;
  word-break: break-word;
}
.bubble.assistant {
  border-color: var(--violet-line);
  background: var(--violet-bg);
}
.bubble.queued {
  border-style: dashed;
  color: var(--text2);
}
.bubble.dropped {
  border-color: var(--red-line);
  background: var(--red-bg2);
}
/* The images the owner sent with his words, under them, at a size that reads in the column. */
.shots {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 6px;
  margin-top: 4px;
}
.shot {
  max-width: min(240px, 100%);
  max-height: 240px;
  border: 1px solid var(--line2);
  border-radius: 8px;
  object-fit: contain;
  background: var(--panel2);
}
/* What a mentor said to the session of the card they help: their words, headed with the mentor mark and their name, on a surface of
   its own, as the owner's words are in theirs. */
.said {
  margin: 8px 0;
  padding: 6px 10px 8px;
  border-radius: 8px;
  background: var(--panel2);
}
.said-h {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}
.said-who {
  min-width: 0;
  overflow: hidden;
  font-size: 12px;
  font-weight: 600;
  color: var(--text);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.said-time {
  margin-left: auto;
  font-size: 11px;
  color: var(--mut);
}
.said-text {
  margin-top: 4px;
  color: var(--text);
  font-size: var(--size-read);
  line-height: var(--leading-read);
  white-space: pre-wrap;
  word-break: break-word;
}
/* What the Chair or the policy decided about a risky request, or who ended the turn: one quiet line, its mark in the tone
   of what was decided. */
.note {
  display: flex;
  align-items: baseline;
  gap: 6px;
  margin: 6px 0;
  font-size: 11px;
  color: var(--faint);
}
/* Where an ask that waits on the owner was raised: a record only, its answers are pinned under the stream (AgentDrawer). */
.note.ask {
  color: var(--orange-text);
  font-size: 12px;
}
/* An agent's risky request, asked or decided, and a run of instructions files read: one quiet line (who it waits on or who
   decided it, and why; the files), the reason in full and the whole request when opened. The line is 24px tall inside
   (WCAG 2.5.8). */
.decision {
  margin: 6px 0;
  font-size: 11.5px;
}
.decision-h {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  min-width: 0;
  min-height: 24px;
  border-radius: 4px;
  text-align: left;
  cursor: pointer;
}
.decision .why {
  margin: 2px 0 0 20px;
  color: var(--mut);
  line-height: 1.5;
}
/* Where one session of the Assistant ends and the next begins: a rule across the stream, titled in the Assistant's violet. */
.session-break {
  margin: 18px 0 14px;
  text-align: center;
}
.session-break .rule {
  height: 1px;
  flex: 1;
  background: var(--violet-line);
}
.session-break .text {
  margin: 6px 0 0;
  color: var(--mut);
  font-size: 11.5px;
}
.prompt-text {
  max-height: 280px;
  margin-top: 6px;
  overflow: auto;
  padding: 8px 10px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--bg);
  color: var(--text2);
  font-size: 12px;
  line-height: 1.6;
}
/* A thought: its label and first words, open while it is being written. */
.thought {
  margin: 6px 0;
}
.think {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: 4px;
  font-size: 11.5px;
  font-weight: 600;
  letter-spacing: 0.3px;
  color: var(--violet);
}
.message {
  position: relative;
  margin: 6px 0;
}
/* The reply's bar (teach it, copy it) shows on the message under the pointer, or while the keyboard is on the bar, so it
   does not crowd the text it sits on. */
.message:hover :deep(.teach-bar),
.message :deep(.teach-bar:focus-within) {
  opacity: 1;
}
/* Tool calls folded into one line, each call a card when opened. */
.tools {
  display: flex;
  align-items: center;
  gap: 7px;
  width: 100%;
  min-height: 24px;
  padding: 3px 0;
  color: var(--mut);
  font-size: 12px;
  text-align: left;
  cursor: pointer;
}
.tools:hover {
  color: var(--text2);
}
.kind {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 15px;
  height: 15px;
  margin-left: -5px;
  border: 1px solid var(--line2);
  border-radius: 5px;
  background: var(--panel2);
}
.kind:first-child {
  margin-left: 0;
}
</style>
