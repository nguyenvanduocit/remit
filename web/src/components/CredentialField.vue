<script setup lang="ts">
import { ref } from 'vue';
import { attempt, messageOf } from '../act';
import { useApi } from '../core/api';
import type { InboxItem } from '../core/generated/types.generated';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

// The one place the owner types into a form: the value of a variable a source's connection needs, which told to an agent
// would enter its context. A secret is typed masked; a plain one (an address, a user) is typed in the clear. The value goes to the server in the call that saves it and nowhere else (not the address, storage, a
// log or an error message), and the box is emptied the moment it is sent, whether or not the server takes it.
const props = defineProps<{ item: InboxItem }>();
const api = useApi();
const value = ref('');
const sending = ref(false);

async function save() {
  const secret = value.value;
  if (!secret || sending.value) return;
  sending.value = true;
  await attempt(
    async () => {
      const answered = api.inboxAnswer(props.item.id, '', secret);
      value.value = '';
      try {
        return await answered;
      } catch (e) {
        throw new Error(messageOf(e).replaceAll(secret, '…'));
      }
    },
    'Credential saved',
    'Could not save the credential',
  );
  sending.value = false;
}
</script>

<template>
  <div>
    <div class="flex items-center gap-1.5">
      <Input v-model="value" :type="item.secret ? 'password' : 'text'" autocomplete="off" size="sm" class="flex-1" :placeholder="item.variable ? `Value of ${item.variable}` : 'Value'" :aria-label="item.variable ? `Value of ${item.variable}` : 'Credential value'" :disabled="sending" @keydown.enter.prevent="save" />
      <Button size="sm" :disabled="!value || sending" @click="save">Save</Button>
    </div>
    <p class="mt-1 text-[11.5px] text-mut">{{ item.secret ? 'Stored sealed and shown nowhere again; no agent is told it.' : 'Stored as typed and shown on the Tools page; no agent is told it.' }}</p>
  </div>
</template>
