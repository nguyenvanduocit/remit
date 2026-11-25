<script setup lang="ts">
import { computed } from 'vue';
import { curlOf, emitOf, keyFileOf } from '../eventSignals';
import CodeBlock from './CodeBlock.vue';

// How a writer writes to the open event signal, for someone doing it once: an outside system posting to its door, a script
// through `remit emit`, and a system that cannot post at all, which a sensor reads instead. The examples carry the
// signal's address and name, and the curl one `<key>` where the key goes until the key above is shown (`revealed`), which is
// then in it; the CLI's reads the key from a file, so it never carries the key.
const props = defineProps<{ name: string; address: string; revealed: string }>();
const example = computed(() => curlOf(props.address, props.revealed || '<key>'));
const cli = computed(() => emitOf(location.origin, props.name));
</script>

<template>
  <section aria-labelledby="event-signal-guide" data-guide>
    <h2 id="event-signal-guide" class="caps mb-1.5">How to write to it</h2>
    <ol class="m-0 flex list-decimal flex-col gap-2.5 pl-5 text-text2" data-guide-body>
      <li data-step><b>GitLab.</b> In the project open Settings → Webhooks. Paste the address as the URL and the key as the Secret token. Tick pipeline events, which the rules that ship read (tick more only for subscriptions of your own), press Add webhook, then Test.</li>
      <li data-step><b>GitHub.</b> In the repository open Settings → Webhooks → Add webhook. Paste the address as the Payload URL, pick <code class="code-chip">application/json</code>, and paste the key as the Secret: GitHub signs each post with it and never sends it. Pick the event workflow runs, which the rules that ship read (pick more only for subscriptions of your own), then Add webhook; GitHub pings it at once.</li>
      <li data-step>
        <b>Any other sender</b> (Sentry, Jira, a script) posts its JSON to the address with the key as <code class="code-chip">Authorization: Bearer &lt;key&gt;</code>, or as <code class="code-chip">X-Gitlab-Token: &lt;key&gt;</code>. A sender that cannot set headers puts it in the address instead: <code class="code-chip">?token=&lt;key&gt;</code>.
        <div class="mt-2"><CodeBlock :text="example" label="Copy the curl example" /></div>
        <p class="mt-1.5 text-mut">The example is for this signal; <code class="code-chip">&lt;key&gt;</code> stands for its key until you press Show key.</p>
      </li>
      <li data-step>
        <b>From a shell or a script,</b> <code class="code-chip">remit emit</code> posts a file (or stdin) to the door and says it came by the CLI. Put the key alone in <code class="code-chip">{{ keyFileOf(name) }}</code>, readable only by you (<code class="code-chip">chmod 600</code>): it refuses a key file others can read, and never takes the key as an argument. <code class="code-chip">REMIT_EVENT_KEY</code> holds it instead where a file cannot.
        <div class="mt-2"><CodeBlock :text="cli" label="Copy the remit emit example" data-cli /></div>
      </li>
      <li data-step><b>A system that cannot post</b> (a GitHub issue opened, a Datadog monitor that alerts) is read by a sensor: ask the Assistant to add one to this signal, naming the source and what to read. A sensor it makes waits for your Apply in its inbox before anything is read.</li>
      <li data-step><b>What comes back.</b> 202 and <code class="code-chip">{"event":"event:&lt;signal&gt;#&lt;rev&gt;"}</code>: the event is stored, and appears on the Events tab. 401: the key does not match this signal. A signal that is paused or deleted answers 401 as well.</li>
      <li data-step><b>The address must be reachable from the sender.</b> A sender outside your network cannot reach an address that only your network can.</li>
    </ol>
  </section>
</template>

<style scoped>
b {
  font-weight: 600;
  color: var(--text);
}
/* A header or a reply stays whole on its line, so it reads as what is typed. */
code {
  white-space: nowrap;
}
</style>
