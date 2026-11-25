<script setup lang="ts">
import { computed, ref } from 'vue';
import Icon from './Icon.vue';
import { attempt, useSend } from '../act';
import { callIn, newPlace } from '../replica';
import type { ChangeKind, InboxItem, InboxOption, PermissionRisk, PermissionTrait } from '../core/generated/types.generated';
import Markdown from './Markdown.vue';
import Callout from './Callout.vue';
import CodeBlock from './CodeBlock.vue';
import Fold from './Fold.vue';
import HoldButton from './HoldButton.vue';
import CredentialField from './CredentialField.vue';
import InboxDecided from './InboxDecided.vue';
import LoginField from './LoginField.vue';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

// One thing waiting on the owner (the Chair's question, an agent asking leave, a turn that failed, files a deleted chain
// left, a credential a source needs, an account to sign in, a change an agent asked to write, a key result that could not be served or judged,
// a subscription held in place on a firing that failed for good), with the answers the server offers; a credential is answered in its own field, which
// comes after the answers the server offers (taking away what is stored). A held subscription's item leads to its line on the
// Subscriptions tab, where the firing it is held on and its error are. An item no longer open waits on no one: it reads as
// decided, with who decided and what (InboxDecided), and offers no answer.
const props = defineProps<{ item: InboxItem }>();
// An answer button asks once at a time: pressing it again while the answer is on its way does nothing, and pressing it again
// after an answer that never came (the connection went) is a retry of that answer with its request id, so the server acts once.
const answering = ref(false);
const place = newPlace('answer');
async function answer(o: InboxOption, text = '') {
  if (answering.value) return;
  answering.value = true;
  await attempt(() => callIn(place, o.id + text, { method: 'inbox_answer', args: { itemId: props.item.id, optionId: o.id, text } }), refuses(o) && text ? 'Refused with your reason' : o.name);
  answering.value = false;
}
// An agent asking leave may be refused in the owner's words: typed, they turn each refusal into "Refuse with this reason" and
// go with it, and the session reads "Refused by you. <words>" (ADR-0080). Enter in the box refuses with them.
const refuses = (o: InboxOption) => !!o.kind?.startsWith('reject');
const reason = ref('');
const because = () => reason.value.trim();
const nameOf = (o: InboxOption) => (refuses(o) && because() ? 'Refuse with this reason' : o.name);
const pick = (o: InboxOption) => answer(o, refuses(o) ? because() : '');
function refuseWithReason() {
  const o = props.item.options.find(refuses);
  if (o && because()) void answer(o, because());
}
// A question can always be answered in the owner's own words: Other swaps the buttons for a field and Send. Whether the field
// is open is view state; the answer is the same call as a button's, with no option and the text.
const typing = ref(false);
const reply = ref('');
const { sending, send } = useSend(reply, (text) => ({ method: 'inbox_answer', args: { itemId: props.item.id, optionId: '', text } }), 'Could not send the answer');
// What the owner reads first and the icon beside it. A change says what it changes when it changes a kind of thing of its own
// (an objective, a hire, a memory), and "A change" for the rest.
const CHANGES: Partial<Record<ChangeKind, { heading: string; icon: string }>> = {
  objective: { heading: 'An objective waits for you', icon: 'chart' },
  employee: { heading: 'A hire waits for you', icon: 'plus' },
  forget: { heading: 'A memory change waits for you', icon: 'trash' },
  promote: { heading: 'A memory to share waits for you', icon: 'person' },
};
const look = (i: InboxItem): { heading: string; icon: string } => {
  switch (i.kind) {
    case 'question':
      return { heading: 'The Chair asks you', icon: 'help' };
    case 'permission':
      return { heading: 'Agent requests permission', icon: 'pause' };
    case 'credential':
      return { heading: 'A credential is needed', icon: 'key' };
    case 'login':
      return { heading: 'An account needs signing in', icon: 'key' };
    case 'exception':
      return { heading: 'An exception waits for you', icon: 'alert' };
    case 'held':
      return { heading: 'A subscription waits for you', icon: 'alert' };
    case 'change':
      return (i.changeKind && CHANGES[i.changeKind]) || { heading: 'A change waits for you', icon: 'pencil' };
    case 'error':
      return { heading: i.cardId ? 'Agent error' : 'Remit could not finish', icon: 'alert' };
  }
};
// The server's own words, for the owner to read; what is not (a command, an error's output) is shown as it is. A held item's
// words carry the error its processor answered, which came from outside Remit: text and only text, its lines kept, so no
// link, image or emphasis in it is ever made.
const prose = (i: InboxItem) => i.kind === 'question' || i.kind === 'credential' || i.kind === 'login' || i.kind === 'change' || i.kind === 'exception';
// The owner may answer a question in his own words (Other).
const ownWords = (i: InboxItem) => i.kind === 'question';
// A permission the server put in words (PermissionAsk): its action is the heading, open or decided; its reason, its traits and
// how careful to be are the body, and the exact call is folded under them, for the one who wants to read it as sent.
const asked = (i: InboxItem) => (i.kind === 'permission' ? i.permission : undefined);
const p = computed(() => asked(props.item));
// What the item asks, as the heading says it once it is decided.
const open = (i: InboxItem) => i.status === 'open';
const heading = (i: InboxItem) => asked(i)?.action ?? (open(i) ? look(i).heading : i.status === 'cancelled' ? 'Withdrawn' : 'Decided');
// The heading says the state in words when the action is the heading: the region is named by it, and the chip's icon shows it.
const named = (i: InboxItem) => {
  const a = asked(i);
  return a ? `${open(i) ? 'Asks your leave' : i.status === 'cancelled' ? 'Withdrawn' : 'Decided'}: ${a.action}` : heading(i);
};
// A trait's badge reads the server's words beside an icon of its kind, so it never rests on colour.
const ICON: Record<PermissionTrait, string> = {
  irreversible: 'trash',
  secrets: 'key',
  production: 'release',
  unsandboxed: 'gate',
  outside_worktree: 'file',
  network: 'external',
};
// How careful to be, the one badge with a hue: red for what cannot be undone, amber to watch, neutral for a plain read.
const HUE: Record<PermissionRisk, 'danger' | 'warning' | 'quiet'> = { high: 'danger', medium: 'warning', low: 'quiet' };
// An answer that lets a high-risk call through is held, never clicked (HoldButton): the server says high cannot be undone.
const held = (i: InboxItem, o: InboxOption) => asked(i)?.risk.kind === 'high' && !!o.kind?.startsWith('allow');
const variant = (o: InboxOption) => (o.id === 'retry' || o.id === 'apply' || o.kind?.startsWith('allow') ? 'success' : o.kind?.startsWith('reject') ? 'destructive' : 'outline');
</script>

<template>
  <Callout :tone="!open(item) ? 'note' : item.kind === 'error' ? 'error' : 'ask'" :icon="look(item).icon" role="region" :aria-label="named(item)" :data-status="item.status" data-row>
    <template #title>{{ heading(item) }}<template v-if="!asked(item) && (item.kind !== 'question' || !open(item))"> · {{ item.title }}</template></template>
    <InboxDecided v-if="!open(item)" :item="item" />
    <template v-else>
      <template v-if="p">
        <p v-if="p.said" class="m-0 mb-1 text-xs text-text2 [overflow-wrap:anywhere]" data-said>The agent says: “{{ p.said }}”</p>
        <p class="m-0 text-read text-text2 [overflow-wrap:anywhere]" data-reason>{{ p.reason }}</p>
        <div class="mt-2 flex flex-wrap items-center gap-1.5" data-traits>
          <Badge :variant="HUE[p.risk.kind]" data-risk><Icon name="alert" :size="11" />{{ p.risk.says }}</Badge>
          <Badge v-for="t in p.traits" :key="t.kind" variant="quiet" :data-trait="t.kind"><Icon :name="ICON[t.kind]" :size="11" />{{ t.says }}</Badge>
        </div>
        <Fold class="mt-1.5">
          <template #summary><span class="text-[12px] text-text2" data-call>The exact call · {{ p.tool }}</span></template>
          <CodeBlock class="mt-1" :text="p.command" label="Copy the call" />
        </Fold>
      </template>
      <Markdown v-else-if="prose(item)" type="body" :text="item.detail" :wrap="item.kind === 'change'" />
      <p v-else-if="item.kind === 'held'" class="text-read whitespace-pre-line text-text [overflow-wrap:anywhere]" data-detail>{{ item.detail }}</p>
      <pre v-else-if="item.detail">{{ item.detail }}</pre>
      <RouterLink v-if="item.kind === 'held' && item.subscription" class="mt-1.5 inline-flex items-center gap-1 text-[12px] text-blue-text hover:underline" :to="{ name: 'subscriptions', params: { entry: item.subscription } }" data-held-line>Open its line on the Subscriptions tab<Icon name="arrow-right" :size="11" /></RouterLink>
      <form v-if="typing" class="mt-2 flex items-center gap-1.5" @submit.prevent="send">
        <Input v-model="reply" autofocus autocomplete="off" size="sm" class="flex-1" aria-label="Your answer" placeholder="Your answer" :disabled="sending" />
        <Button type="submit" size="sm" :disabled="!reply.trim() || sending">Send</Button>
      </form>
      <Input
        v-if="item.kind === 'permission'"
        v-model="reason"
        autocomplete="off"
        size="sm"
        class="mt-2"
        aria-label="Your reason to refuse"
        placeholder="Your reason to refuse, if you refuse"
        :disabled="answering"
        data-refuse-reason
        @keydown.enter.exact.prevent="refuseWithReason"
      />
      <div v-if="!typing && (item.options.length || ownWords(item))" class="mt-2 flex flex-wrap items-center gap-1.5">
        <template v-for="o in item.options" :key="o.id">
          <HoldButton v-if="held(item, o)" :label="o.name" variant="success" :disabled="answering" :data-option="o.id" @confirm="pick(o)" />
          <Button v-else :variant="variant(o)" :disabled="answering" :data-option="o.id" @click="pick(o)">{{ nameOf(o) }}</Button>
        </template>
        <Button v-if="ownWords(item)" variant="outline" @click="typing = true">Other</Button>
      </div>
      <CredentialField v-if="item.kind === 'credential'" :item="item" class="mt-2" />
      <LoginField v-else-if="item.kind === 'login'" :item="item" />
    </template>
  </Callout>
</template>
