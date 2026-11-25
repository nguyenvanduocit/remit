<script setup lang="ts">
import { resend, unsend } from '../drafts';
import type { Pending } from '../core/generated/replica/types.generated';
import { Button } from '@/components/ui/button';
import Icon from './Icon.vue';
import Spinner from './Spinner.vue';

// A box's sends the server has not taken yet (ADR-0094), where its caller shows them: the words as sent, and under them what is
// happening to them in the core's words. A send asked a moment ago says nothing; one still unanswered after a moment says it
// is sending; with the link down it says it goes when the connection is back; one the server took says so for a moment; one
// that failed says why, with its one way out: try it again, or take the words back into the box to edit them.
defineProps<{ pending: Pending[] }>();
</script>

<template>
  <ul v-if="pending.length" class="flex flex-col items-end gap-1.5 pb-2" aria-label="Messages on their way">
    <li v-for="p in pending" :key="p.id" class="flex max-w-[85%] flex-col items-end gap-1" :data-pending="p.status">
      <p class="bubble whitespace-pre-wrap px-3 py-2 text-[13px] leading-relaxed" :class="p.status === 'failed' ? 'border-red-line' : 'border-line2'">{{ p.text }}</p>
      <p v-if="p.status === 'waiting'" class="flex items-center gap-1 text-[11.5px] text-mut" role="status"><Icon name="clock" :size="12" />Goes out when the connection is back</p>
      <p v-else-if="p.status === 'sent'" class="flex items-center gap-1 text-[11.5px] text-mut" role="status"><Icon name="check" :size="12" />Sent</p>
      <p v-else-if="p.status === 'slow'" class="flex items-center gap-1.5 text-[11.5px] text-mut" role="status"><Spinner :size="10" />Sending…</p>
      <div v-else-if="p.status === 'failed'" class="flex flex-wrap items-center justify-end gap-1.5 text-[11.5px] text-red-text" role="alert">
        <Icon name="alert" :size="12" /><span>Not sent: {{ p.why }}</span>
        <Button variant="outline" size="xs" data-resend @click="resend(p.id)"><Icon name="repeat" :size="11" />Try again</Button>
        <Button variant="ghost" size="xs" data-unsend @click="unsend(p.id)"><Icon name="pencil" :size="11" />Edit</Button>
      </div>
    </li>
  </ul>
</template>

<style scoped>
.bubble {
  border-width: 1px;
  border-radius: 12px;
  background: var(--panel2);
  color: var(--text);
}
</style>
