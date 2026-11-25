<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import { useAct } from '../act';
import { useApi } from '../core/api';
import { catalog, chainRecords } from '../catalog';
import { CLIENT_ENDPOINT, CLIENT_KEY_MASK, NEW_CLIENT_KEY_WORDS } from '../clientKeys';
import { useSayToAssistant } from '../sayToAssistant';
import { actorLabel } from '../stream';
import Ago from './Ago.vue';
import CodeBlock from './CodeBlock.vue';
import EmptyPage from './EmptyPage.vue';
import EmptyState from './EmptyState.vue';
import Icon from './Icon.vue';
import KeyReveal from './KeyReveal.vue';
import KeyValues from './KeyValues.vue';
import ListPage from './ListPage.vue';
import NavRow from './NavRow.vue';
import PageBody from './PageBody.vue';
import PageSection from './PageSection.vue';
import PageTabs from './PageTabs.vue';
import StatusChip from './StatusChip.vue';
import Tip from './Tip.vue';
import { Button } from '@/components/ui/button';
import PageGrid from './PageGrid.vue';

// The keys the owner handed to outside agents, newest made first (catalog.clientKeys, as the server lists them, never with a
// value): the open one has its limits, the chains it started, how its holder reaches the endpoint and its value (masked until
// he asks, KeyReveal). A key is made, limited, rekeyed and deleted by asking the Assistant, so New client key opens it with the
// sentence started in its box; pausing and resuming are the owner's rails, buttons here. With no key yet, the page says what one
// is and how it comes (EmptyPage).
const route = useRoute();
const api = useApi();
const act = useAct();
const sayTo = useSayToAssistant();
const key = computed(() => catalog.clientKeys.find((k) => k.id === route.params.keyId) ?? catalog.clientKeys[0]);
// The chains the open key started, newest first as the catalog keeps them.
const started = computed(() => chainRecords.value.filter((c) => c.by?.kind === 'client' && c.by.who === key.value?.id));
const setup = computed(() => [
  'install -m 600 /dev/null ~/.config/remit/client.key   # then paste the key into it',
  '# .mcp.json: Claude Code runs the headersHelper for each connection, so the key never sits in a config or an argument',
  '{ "mcpServers": { "remit-client": {',
  '  "type": "http", "url": "http://127.0.0.1:<port>' + CLIENT_ENDPOINT + '",',
  `  "headersHelper": "jq -nc --rawfile k ~/.config/remit/client.key '{Authorization: (\\"Bearer \\" + (\$k | rtrimstr(\\"\\\\n\\")))}'"`,
  '} } }',
].join('\n'));
const newKey = () => sayTo(NEW_CLIENT_KEY_WORDS);
</script>

<template>
  <ListPage name="clientKeys" open :title="key?.name" mono>
    <template #tabs><PageTabs group="integrations" /></template>
    <template #head>
      <Button class="w-full" variant="outline" size="sm" data-new @click="newKey()"><Icon name="plus" :size="12" /> New client key</Button>
    </template>
    <template #list>
      <NavRow v-for="k in catalog.clientKeys" :key="k.id" :on="key?.id === k.id" :to="{ name: 'clientKeys', params: { keyId: k.id } }" :description="k.note" :data-key-id="k.id" :data-paused="k.paused ? '' : undefined">
        <template #title><span class="min-w-0 font-mono [overflow-wrap:anywhere]">{{ k.name }}</span><StatusChip v-if="k.paused" status="paused" class="flex-none" data-paused-badge /></template>
      </NavRow>
    </template>
    <template v-if="key" #standing>
      <StatusChip v-if="key.paused" status="paused" class="flex-none" data-paused-badge />
    </template>
    <template v-if="key" #actions>
      <Tip v-if="key.paused" text="Let its holder in again"><Button variant="outline" size="sm" :aria-label="`Resume client key ${key.name}`" data-resume @click="act('clientKeyResume', [key.id], 'Client key resumed')"><Icon name="play" :size="12" /> Resume</Button></Tip>
      <Tip v-else text="Turn its holder away (401) until you resume it. Chains it started go on."><Button variant="outline" size="sm" :aria-label="`Pause client key ${key.name}`" data-pause @click="act('clientKeyPause', [key.id], 'Client key paused')"><Icon name="pause" :size="12" /> Pause</Button></Tip>
    </template>
    <EmptyPage v-if="!key" title="No client key yet" icon="key" :say="`${NEW_CLIENT_KEY_WORDS}evaluator`">
      <template #what>A client key is the way in for one outside agent (an evaluator, a script): with it, that agent reads what Remit knows, starts a limited number of work chains, which hire no engineer, and pauses or messages what runs. The endpoint answers this machine only.</template>
      <template #how>
        <li>Hand one out here: New client key starts the sentence in the Assistant's box, and you finish it with a name.</li>
        <li>Tell the Assistant how many chains a day it may start (0 for none); its chains hire no engineer.</li>
        <li>Put the key in a file only its holder reads, as the key's guide shows; the chains it starts show on the Board.</li>
      </template>
    </EmptyPage>
    <PageBody v-else>
      <!-- The cards fill the page; wide enough (two cards of a readable width), they stand two abreast, each its own height;
           what is wide in itself (a flow, a guide with its command) takes the whole row. -->
      <div class="@container">
      <PageGrid>
        <PageSection title="What it is for" id="key-about" data-about>
          <p v-if="key.note" class="mb-3.5 text-text2 [overflow-wrap:anywhere]" data-note>{{ key.note }}</p>
          <KeyValues>
            <dt>{{ key.updatedAt === key.createdAt ? 'Made' : 'Changed' }}</dt><dd data-by>{{ actorLabel(key.by) }} · <Ago :at="key.updatedAt" /></dd>
            <dt>Last used</dt><dd data-last-used><template v-if="key.lastUsedAt"><Ago :at="key.lastUsedAt" /> from {{ key.lastAddr }}</template><template v-else>Never</template></dd>
            <dt>Its holder</dt><dd>{{ key.paused ? 'Turned away (401) until it is resumed' : 'Let in with its key' }}</dd>
          </KeyValues>
        </PageSection>
        <PageSection title="Limits" id="key-limits">
          <KeyValues data-limits>
            <dt>Chains a day</dt><dd>{{ key.chainsPerDay }}</dd>
            <dt>Acts an hour</dt><dd>{{ key.actionsPerHour }} (starting and steering together)</dd>
            <dt>May start</dt><dd data-may-start>{{ key.chainsPerDay ? 'Work chains, which hire no engineer' : 'Nothing: tell the Assistant how many chains a day it may start' }}</dd>
          </KeyValues>
        </PageSection>
        <PageSection title="Chains it started" id="key-chains" :count="started.length">
          <ul v-if="started.length" class="m-0 flex list-none flex-col gap-1.5 p-0" data-chains>
            <li v-for="c in started" :key="c.id" class="flex items-center gap-2" data-chain>
              <RouterLink :to="{ name: 'chain', params: { id: c.id } }" class="hit min-w-0 text-blue-text hover:underline [overflow-wrap:anywhere]">{{ c.title }}</RouterLink>
              <StatusChip :status="c.status" class="flex-none" />
            </li>
          </ul>
          <EmptyState v-else data-chains-none>It started no chain yet.</EmptyState>
        </PageSection>
        <KeyReveal :key="key.id" :name="key.name" :mask="CLIENT_KEY_MASK" :read="() => api.clientKey(key!.id)" />
        <PageSection title="How its holder connects" id="key-guide" class="@[1100px]:col-span-2" data-guide>
          <p class="mb-2 text-text2">The endpoint answers this machine only. Put the key in a file only its holder reads (mode 600), never in a prompt, a message or an argument, and let the process that connects read the file.</p>
          <CodeBlock :text="setup" label="Copy the example" />
        </PageSection>
      </PageGrid>
      </div>
    </PageBody>
  </ListPage>
</template>
