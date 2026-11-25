<script setup lang="ts">
import { ref } from 'vue';
import { attempt, messageOf } from '../act';
import { useApi } from '../core/api';
import type { InboxItem } from '../core/generated/types.generated';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

// Signing an account in: the link opens Anthropic's sign-in in a new tab, and the code its page shows is typed here. The
// code goes to the server in the call that sends it and nowhere else (not the address, storage, a log or an error
// message), and the box is emptied the moment it is sent, whether or not the server takes it. The item closes by itself
// when the sign-in ends.
const props = defineProps<{ item: InboxItem }>();
const api = useApi();
const code = ref('');
const sending = ref(false);

async function send() {
  const secret = code.value.trim();
  if (!secret || sending.value) return;
  sending.value = true;
  await attempt(
    async () => {
      const answered = api.inboxAnswer(props.item.id, '', secret);
      code.value = '';
      try {
        return await answered;
      } catch (e) {
        throw new Error(messageOf(e).replaceAll(secret, '…'));
      }
    },
    'Code sent',
    'Could not send the code',
  );
  sending.value = false;
}
</script>

<template>
  <div class="mt-2">
    <Button v-if="item.link" as="a" size="sm" variant="outline" :href="item.link" target="_blank" rel="noopener noreferrer">Open the sign-in page</Button>
    <div class="mt-2 flex items-center gap-1.5">
      <Input v-model="code" type="password" autocomplete="off" size="sm" class="flex-1" placeholder="Code from the sign-in page" aria-label="Code from the sign-in page" :disabled="sending" @keydown.enter.prevent="send" />
      <Button size="sm" :disabled="!code.trim() || sending" @click="send">Send</Button>
    </div>
    <p class="mt-1 text-[11.5px] text-mut">Given to the sign-in and shown nowhere again; no agent is told it.</p>
  </div>
</template>
