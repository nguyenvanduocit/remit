<script setup lang="ts">
import { computed, onBeforeUnmount, ref, useId } from 'vue';
import { useTimeoutFn } from '@vueuse/core';
import { messageOf } from '../act';
import { KEY_SHOWN_MS } from '../keys';
import CodeBlock from './CodeBlock.vue';
import CopyButton from './CopyButton.vue';
import { copyText } from '../clipboard';
import Icon from './Icon.vue';
import Notice from './Notice.vue';
import Tip from './Tip.vue';
import { Button } from '@/components/ui/button';

// One key (an event signal's, a client key's), masked until the owner asks for it. The key is no part of the record the server
// lists: `read` asks the server for it, and it lives only here, for 30 seconds or until he hides it or this goes; this tells
// its parent (`revealed`) so a guide's example can carry it while it is shown. The page keys this by the record, so a key
// never outlives the record it was read for. `name` names the record for the labels, `mask` stands in for the key, `noun` is
// what the page calls it (a key unless it says otherwise: a person's code). `link` makes the address that signs in with the
// key, offered to copy beside it (a mentor's code: the link the owner passes on).
const props = withDefaults(defineProps<{ name: string; mask: string; read: () => Promise<{ key: string }>; noun?: string; link?: (key: string) => string }>(), { noun: 'key', link: undefined });
const titleId = useId();
const Noun = computed(() => props.noun.charAt(0).toUpperCase() + props.noun.slice(1));
const emit = defineEmits<{ revealed: [key: string] }>();

const key = ref('');
const failure = ref('');
const asking = ref(false);
const { start, stop } = useTimeoutFn(() => hide(), KEY_SHOWN_MS, { immediate: false });
let gone = false;
function hide(): void {
  stop();
  if (!key.value) return;
  key.value = '';
  emit('revealed', '');
}
async function show(): Promise<void> {
  asking.value = true;
  failure.value = '';
  try {
    const answer = await props.read();
    if (gone) return;
    key.value = answer.key;
    emit('revealed', answer.key);
    start();
  } catch (e) {
    if (!gone) failure.value = messageOf(e);
  } finally {
    asking.value = false;
  }
}
onBeforeUnmount(() => {
  gone = true;
  hide();
});
</script>

<template>
  <section :aria-labelledby="titleId">
    <h2 :id="titleId" class="caps mb-1.5">{{ Noun }}</h2>
    <CodeBlock v-if="key" :text="key" :label="`Copy ${noun} of ${name}`" data-key>
      <template #actions>
        <CopyButton :text="key" :label="`Copy ${noun} of ${name}`" />
        <Tip v-if="link" text="The address that signs in with it: they open it, or paste the code on the sign-in page"><Button variant="outline" size="xs" class="flex-none" :aria-label="`Copy the sign-in link of ${name}`" data-copy-link @click="copyText(link(key))"><Icon name="link" :size="12" /> Copy link</Button></Tip>
        <Tip text="Hide it now. It hides by itself after 30 seconds."><Button variant="outline" size="xs" class="flex-none" :aria-label="`Hide ${noun} of ${name}`" data-hide-key @click="hide()">Hide</Button></Tip>
      </template>
    </CodeBlock>
    <CodeBlock v-else :text="mask" :label="`${Noun} of ${name}, hidden`" data-key-mask>
      <template #actions>
        <Tip :text="`Read the ${noun} from the server and show it for 30 seconds`"><Button variant="outline" size="xs" class="flex-none" :disabled="asking" :aria-label="`Show ${noun} of ${name}`" data-show-key @click="show()"><Icon name="key" :size="12" /> Show {{ noun }}</Button></Tip>
      </template>
    </CodeBlock>
    <Notice v-if="failure" tone="danger" class="mt-2" data-key-error>{{ failure }}</Notice>
  </section>
</template>
