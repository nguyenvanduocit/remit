<script setup lang="ts">
import { onMounted, ref, useId } from 'vue';
import { google, loadGoogle, signInWithGoogle } from '../google';
import { session } from '../session';
import { comeInWith } from '../wayIn';
import Callout from './Callout.vue';
import Icon from './Icon.vue';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

// The one screen before the app: how the owner comes in, with Google when the server offers it (ADR-0105) or with the
// owner key, and how a mentor comes in with the code the owner passed on (the link that carries it, or the code pasted
// here). When the server refused this browser's credential or a Google account, it says so, as every error is shown
// (Callout).
const code = ref('');
const codeId = useId();
function submit(): void {
  if (code.value.trim()) comeInWith(code.value);
}
onMounted(() => void loadGoogle());
</script>

<template>
  <div class="fixed inset-0 flex overflow-auto bg-bg p-4">
    <div class="m-auto flex w-full max-w-[380px] flex-col gap-3 rounded-xl border border-line bg-panel p-5 sm:p-[22px] text-[12.5px]/normal shadow-(--shadow-raised)">
      <div class="flex items-center gap-2 text-base text-text"><Icon class="text-blue-mark" name="link" :size="18" /><h1 class="font-semibold">Remit</h1></div>
      <Callout v-if="session.auth === 'rejected'" tone="error" data-rejected>This browser's key or code does not open Remit: it was mistyped, the owner stopped it, its Google sign-in ended, or the server is not running.</Callout>
      <template v-if="google.offered">
        <div class="flex flex-col gap-1.5" data-google>
          <Button :disabled="google.busy || !!google.signedIn" :aria-busy="google.busy" data-google-sign-in @click="signInWithGoogle">
            <Icon name="person" :size="15" />{{ google.busy ? 'Waiting for Google…' : 'Sign in with Google' }}
          </Button>
          <p class="m-0 text-mut">The owner: sign in with the Google account this server lets in.</p>
        </div>
        <p v-if="google.signedIn" class="m-0 text-text" role="status" data-google-signed-in>Signed in as <b>{{ google.signedIn }}</b>: opening Remit…</p>
      </template>
      <Callout v-if="google.error" tone="error" role="alert" data-google-error>{{ google.error }}</Callout>
      <hr v-if="google.offered" class="m-0 border-line" />
      <form class="flex flex-col gap-1.5" data-code-form @submit.prevent="submit">
        <label :for="codeId" class="text-text">Mentor? Paste the code you were given</label>
        <div class="flex gap-2">
          <Input :id="codeId" v-model="code" type="password" autocomplete="off" spellcheck="false" placeholder="wk_…" class="min-w-0 flex-1" data-code />
          <Button type="submit" :disabled="!code.trim()" data-sign-in>Sign in</Button>
        </div>
        <p class="m-0 text-mut">Or open the sign-in link you were sent: it holds your code, and this browser keeps it.</p>
      </form>
      <hr class="m-0 border-line" />
      <p class="m-0 text-mut">The owner{{ google.offered ? ', without Google' : '' }}: open the link Remit printed in its terminal when it started: it holds your owner key, and this browser keeps it.</p>
      <p class="m-0 text-mut">No link at hand? Add <code class="code-chip">#key=</code> and the key in <code class="code-chip">~/.remit/owner.key</code> (in the data directory) to this page's address.</p>
    </div>
  </div>
</template>
