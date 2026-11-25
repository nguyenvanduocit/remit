<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import { useAct } from '../act';
import { capabilityLabel, catalog } from '../catalog';
import { INSTALL_PLUGIN_WORDS, ORIGIN_LABEL } from '../extensions';
import { useSayToAssistant } from '../sayToAssistant';
import { actorLabel } from '../stream';
import { plural } from '../ui';
import Ago from './Ago.vue';
import EmptyPage from './EmptyPage.vue';
import EmptyState from './EmptyState.vue';
import Icon from './Icon.vue';
import KeyValues from './KeyValues.vue';
import ListPage from './ListPage.vue';
import NavRow from './NavRow.vue';
import PageBody from './PageBody.vue';
import PageGrid from './PageGrid.vue';
import PageSection from './PageSection.vue';
import PageTabs from './PageTabs.vue';
import StandingBadge from './StandingBadge.vue';
import Tip from './Tip.vue';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

// The plugins that are sources of tools and events (catalog.extensions, as the server lists them: the ones that ship with
// Remit first, then the ones the owner applied), disabled ones too. The open one says where it came from, what its tools do
// and the grant each needs. Disabling takes a plugin out of the sources until he enables it, and enabling is the owner's rail,
// buttons here. Installing is asking the Assistant to propose a plugin: Install a plugin opens it with the sentence started,
// and the proposal comes to his inbox to read and apply.
const route = useRoute();
const act = useAct();
const sayTo = useSayToAssistant();
const extension = computed(() => catalog.extensions.find((e) => e.id === route.params.extensionId) ?? catalog.extensions[0]);
const install = () => sayTo(INSTALL_PLUGIN_WORDS);
</script>

<template>
  <ListPage name="extensions" open :title="extension?.label">
    <template #tabs><PageTabs group="integrations" /></template>
    <template #head>
      <Button class="w-full" variant="outline" size="sm" data-install @click="install()"><Icon name="plus" :size="12" /> Install a plugin</Button>
    </template>
    <template #list>
      <NavRow v-for="e in catalog.extensions" :key="e.id" :on="extension?.id === e.id" :to="{ name: 'extensions', params: { extensionId: e.id } }" :description="ORIGIN_LABEL[e.origin]" :data-extension-id="e.id" :data-disabled="e.enabled ? undefined : ''">
        <template #title><span class="min-w-0 [overflow-wrap:anywhere]">{{ e.label }}</span><StandingBadge v-if="!e.enabled" variant="quiet" class="flex-none" data-disabled-badge>Disabled</StandingBadge></template>
      </NavRow>
    </template>
    <template v-if="extension" #standing>
      <StandingBadge v-if="!extension.enabled" variant="quiet" class="flex-none" data-disabled-badge>Disabled</StandingBadge>
    </template>
    <template v-if="extension" #actions>
      <Tip v-if="!extension.enabled" text="Offer its tools to agents again"><Button variant="outline" size="sm" :aria-label="`Enable plugin ${extension.label}`" data-enable @click="act('extensionEnable', [extension.id], 'Plugin enabled')"><Icon name="play" :size="12" /> Enable</Button></Tip>
      <Tip v-else text="Take it out of the sources: agents lose its tools until you enable it. It stays installed."><Button variant="outline" size="sm" :aria-label="`Disable plugin ${extension.label}`" data-disable @click="act('extensionDisable', [extension.id], 'Plugin disabled')"><Icon name="pause" :size="12" /> Disable</Button></Tip>
    </template>
    <EmptyPage v-if="!extension" title="No plugin yet" icon="terminal" :say="`${INSTALL_PLUGIN_WORDS}/path/to/plugin`">
      <template #what>A plugin is a source Remit does not ship in its core: a program that serves tools to agents, and events to signals, once you apply it.</template>
      <template #how>
        <li>Tell the Assistant the path of a built plugin; its Chair proposes it and you read what it says it is.</li>
        <li>You apply the proposal in your inbox; its tools are then offered to the chains that hold its capability.</li>
      </template>
    </EmptyPage>
    <PageBody v-else>
      <div class="@container">
        <PageGrid>
          <PageSection title="About" id="extension-about" data-about>
            <KeyValues>
              <dt>Comes from</dt><dd data-origin>{{ ORIGIN_LABEL[extension.origin] }}</dd>
              <dt>Lets agents</dt><dd data-capability>{{ capabilityLabel(extension.capability) }}</dd>
              <template v-if="!extension.enabled"><dt>Disabled by</dt><dd data-standing><template v-if="extension.disabledBy">{{ actorLabel(extension.disabledBy) }}</template><template v-if="extension.disabledAt"> · <Ago :at="extension.disabledAt" /></template></dd></template>
              <template v-if="extension.appliedAt"><dt>Applied</dt><dd data-applied><Ago :at="extension.appliedAt" /></dd></template>
              <template v-if="extension.sha256"><dt>Program</dt><dd class="font-mono [overflow-wrap:anywhere]" data-sha>sha256 {{ extension.sha256 }}</dd></template>
            </KeyValues>
          </PageSection>
          <!-- A row of its own: a plugin may serve many tools. Each tool is a card, two abreast once each has a line's room: its name
               and grant, what it does, then the inputs its schema takes, each with its type, whether it is required, its choices and default. -->
          <PageSection title="Tools" id="extension-tools" :count="extension.tools.length" class="@[1100px]:col-span-2">
            <div v-if="extension.tools.length" class="@container/tools">
              <ul class="m-0 grid list-none items-start gap-3 p-0 @4xl/tools:grid-cols-2" data-tools>
                <li v-for="t in extension.tools" :key="t.name" class="flex min-w-0 flex-col gap-2.5 rounded-lg bg-bg2 px-3.5 py-3" data-tool>
                  <span class="flex items-center gap-2"><code class="min-w-0 font-mono font-medium [overflow-wrap:anywhere]">{{ t.name }}</code><Badge variant="quiet" class="flex-none">{{ t.effect }}</Badge></span>
                  <p class="m-0 text-text2 [overflow-wrap:anywhere]" data-tool-what>{{ t.what }}</p>
                  <div v-if="t.inputs.length" class="flex flex-col gap-1.5" data-tool-inputs>
                    <span class="text-[11.5px] text-mut">{{ plural(t.inputs.length, 'input') }}</span>
                    <dl class="m-0 flex flex-col divide-y divide-line rounded-md border border-line bg-panel">
                      <div v-for="i in t.inputs" :key="i.name" class="flex min-w-0 flex-col gap-1 px-3 py-2" :data-tool-input="i.name">
                        <dt class="flex flex-wrap items-center gap-x-2 gap-y-1">
                          <code class="font-mono [overflow-wrap:anywhere]">{{ i.name }}</code>
                          <span class="font-mono text-[11.5px] text-mut">{{ i.type }}</span>
                          <Badge v-if="i.required" variant="warning" data-required>required</Badge>
                        </dt>
                        <dd v-if="i.what" class="m-0 text-text2 [overflow-wrap:anywhere]">{{ i.what }}</dd>
                        <dd v-if="i.choices.length" class="m-0 flex flex-wrap items-center gap-1 text-[11.5px] text-mut" data-choices>One of <code v-for="c in i.choices" :key="c" class="rounded bg-bg2 px-1 font-mono text-text2">{{ c }}</code></dd>
                        <dd v-if="i.default" class="m-0 text-[11.5px] text-mut" data-default>Default <code class="rounded bg-bg2 px-1 font-mono text-text2">{{ i.default }}</code></dd>
                      </div>
                    </dl>
                  </div>
                  <span v-else class="text-[11.5px] text-mut" data-tool-inputs-none>Takes no inputs.</span>
                </li>
              </ul>
            </div>
            <EmptyState v-else data-tools-none>{{ extension.origin === 'api' ? 'An API behind the proxy has no tools: agents reach it with scripts.' : `It has no tools; ${plural(0, 'tool')}.` }}</EmptyState>
          </PageSection>
        </PageGrid>
      </div>
    </PageBody>
  </ListPage>
</template>
