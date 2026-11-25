<script setup lang="ts">
import { useElementSize } from '@vueuse/core';
import { computed, useTemplateRef } from 'vue';
import { useRoute } from 'vue-router';
import { useApi } from '../core/api';
import { catalog } from '../catalog';
import { workGroupOf, type CrewBoard } from '../crewColumns';
import { fileNames, heldBy, openFile, sourceText } from '../library';
import { fileList, fileListAbove, MAIN_MIN_PERCENT, STACK_FILES_BELOW } from '../panels';
import type { ListedSkill } from '../core/generated/types.generated';
import { bytes, plural } from '../ui';
import { useRemote } from '../useRemote';
import Ago from './Ago.vue';
import ChangeHistory from './ChangeHistory.vue';
import CrewPage from './CrewPage.vue';
import EmptyPage from './EmptyPage.vue';
import KeyValues from './KeyValues.vue';
import PageBody from './PageBody.vue';
import PageSection from './PageSection.vue';
import PageSections from './PageSections.vue';
import SkillCard from './SkillCard.vue';
import { Badge } from '@/components/ui/badge';
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable';

// The skills the owner's agents can hold, in the board's shape (CrewPage): every skill a card, in the column of the kind of
// work it is for (ListedSkill.group); the open one says what it is for, who holds it, its files to read, and its
// history. The file read is in the address (?file=), SKILL.md until another is chosen or when the one asked for is not there;
// its text is read from the server as it opens, and again when the skill changes.
const route = useRoute();
const api = useApi();
const openId = computed(() => (typeof route.params.skillName === 'string' ? route.params.skillName : ''));
const skill = computed(() => catalog.skills.find((s) => s.name === openId.value));
const board: CrewBoard<ListedSkill> = { groups: () => catalog.boards?.skills ?? [], rows: () => catalog.skills, groupOf: workGroupOf, keyOf: (s) => s.name, textOf: (s) => `${s.name} ${s.description}` };
const names = computed(() => (skill.value ? fileNames(skill.value.files) : []));
const sizes = computed(() => new Map(skill.value?.files.map((f) => [f.path, bytes(f.size)])));
const file = computed(() => (skill.value ? openFile(skill.value.files, typeof route.query.file === 'string' ? route.query.file : undefined) : ''));
// The files' list and viewer side by side, or the list above the viewer where the page is too narrow to give the file room.
// Measured on a box of its own: the group below is made again when it turns, and one measured on itself would turn back at once.
const filesBox = useTemplateRef<HTMLElement>('filesBox');
const { width: filesWidth } = useElementSize(filesBox);
const stacked = computed(() => filesWidth.value > 0 && filesWidth.value < STACK_FILES_BELOW);
const holders = computed(() => (skill.value ? heldBy(catalog.agents, skill.value.name) : []));
const opened = useRemote(
  () => (skill.value && file.value ? { name: skill.value.name, path: file.value, rev: skill.value.rev } : undefined),
  (a) => api.skillFile(a.name, a.path),
);
// The text shown is the open file's: an answer for another file (or an older version) is not shown while the right one loads.
const text = computed(() => (opened.of.value?.name === skill.value?.name && opened.of.value?.path === file.value && opened.of.value?.rev === skill.value?.rev ? opened.data.value?.text : undefined));
</script>

<template>
  <CrewPage group="crew" name="skills" :board="board" :open-id="openId" :open="!!skill" noun="skills" :title="skill?.name" mono gone="This skill is not listed any more.">
    <template #row="{ row, on }"><SkillCard :skill="row" :on="on" /></template>
    <template #empty>
      <EmptyPage title="No skill yet" icon="file" say="Bring in the skills from the Claude Code configuration on this machine">
        <template #what>A skill is a set of instructions an agent loads when its work calls for them: a SKILL.md that says when to use it, and the files it refers to. An agent holds the skills its definition names, and a card of it takes them along.</template>
        <template #how>
          <li>You tell the Assistant to bring a skill in from the Claude Code configuration on this machine, or to write one.</li>
          <li>It is listed in the column of the kind of work it is for; opening one shows its files and which agents hold it.</li>
          <li>To give it to an agent, or to change it, tell the Assistant again.</li>
        </template>
      </EmptyPage>
    </template>
    <template v-if="skill" #standing>
      <span data-badges class="flex flex-none items-center gap-1.5">
        <Badge variant="quiet">{{ sourceText(skill.source) }}<template v-if="skill.source">&nbsp;· <Ago :at="skill.source.at" /></template></Badge>
        <Badge variant="quiet">{{ plural(names.length, 'file') }}</Badge>
      </span>
    </template>
    <PageBody v-if="skill">
      <PageSections>
        <PageSection title="What it is for" id="skill-about" data-about>
          <p class="mb-3.5 text-text2 [overflow-wrap:anywhere]">{{ skill.description }}</p>
          <KeyValues>
            <dt>Source</dt><dd data-source>{{ sourceText(skill.source) }}<template v-if="skill.source"> · <Ago :at="skill.source.at" /></template></dd>
            <dt>Held by</dt>
            <dd data-held-by>
              <template v-if="holders.length"><template v-for="(a, i) in holders" :key="a.id"><RouterLink class="text-blue-text hover:underline" :to="{ name: 'agents', params: { agentId: a.id } }">{{ a.name }}</RouterLink><template v-if="i < holders.length - 1">, </template></template></template>
              <template v-else>No agent holds it.</template>
            </dd>
          </KeyValues>
        </PageSection>
        <!-- The files take a good share of the window's height, so a long file scrolls inside its viewer and not the page. -->
        <PageSection title="Files" :count="names.length" id="skill-files" flush data-files>
          <div ref="filesBox" class="h-[max(16rem,60vh)]">
            <ResizablePanelGroup :key="stacked ? 'above' : 'beside'" :direction="stacked ? 'vertical' : 'horizontal'" :auto-save-id="stacked ? 'skill-files-above' : 'skill-files'" class="p-2">
              <ResizablePanel id="skill-files-list" :order="1" class="flex" v-bind="stacked ? fileListAbove : fileList">
                <nav class="min-w-0 flex-1 overflow-auto" aria-label="Files of the skill">
                  <RouterLink
                    v-for="f in names" :key="f" :data-file="f" :to="{ name: 'skills', params: { skillName: skill.name }, query: { ...route.query, file: f } }" :aria-current="f === file ? 'true' : undefined"
                    class="flex items-baseline gap-2 rounded-md px-2 py-1.5" :class="f === file ? 'bg-panel3 text-text' : 'text-text2 hover:bg-panel2'"
                  >
                    <span class="mono min-w-0 flex-1 [overflow-wrap:anywhere]">{{ f }}</span>
                    <span class="flex-none text-[11px] text-mut">{{ sizes.get(f) }}</span>
                  </RouterLink>
                </nav>
              </ResizablePanel>
              <ResizableHandle :class="stacked ? 'my-1 bg-transparent' : 'mx-1 bg-transparent'" aria-label="Resize the file list" />
              <ResizablePanel id="skill-files-viewer" :order="2" class="flex" :min-size="MAIN_MIN_PERCENT">
                <pre data-viewer tabindex="0" :aria-label="`${file}, read only`" :aria-busy="text === undefined && !opened.error.value" class="mono min-w-0 flex-1 overflow-auto rounded-lg border border-line bg-bg p-3 whitespace-pre-wrap text-text2 [overflow-wrap:anywhere]"><template v-if="opened.error.value && text === undefined"><span class="text-red-text">Could not read {{ file }}: {{ opened.error.value }}</span></template><template v-else>{{ text }}</template></pre>
              </ResizablePanel>
            </ResizablePanelGroup>
          </div>
        </PageSection>
        <ChangeHistory kind="skill" :name="skill.name" :rev="skill.rev" />
      </PageSections>
    </PageBody>
  </CrewPage>
</template>
