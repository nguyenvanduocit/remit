<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import { catalog, chainRecords, standingOf } from '../catalog';
import { useSettle } from '../nav';
import type { Chain, Project } from '../core/generated/types.generated';
import { fullTime, plural } from '../ui';
import ActorTag from './ActorTag.vue';
import Ago from './Ago.vue';
import ChainCard from './ChainCard.vue';
import ChangeHistory from './ChangeHistory.vue';
import EmptyPage from './EmptyPage.vue';
import EmptyState from './EmptyState.vue';
import Icon from './Icon.vue';
import KeyValues from './KeyValues.vue';
import ListPage from './ListPage.vue';
import Markdown from './Markdown.vue';
import NavRow from './NavRow.vue';
import PageBody from './PageBody.vue';
import PageGrid from './PageGrid.vue';
import PageSection from './PageSection.vue';
import PageTabs from './PageTabs.vue';
import Tip from './Tip.vue';
import { Badge } from '@/components/ui/badge';

// The products the owner's work is on (ADR-0108), by name as the server lists them, archived ones too. The open one
// (/projects/<id>, settled on the first active one, else the first) says who it is for, the owner's notes on it, its repositories and connections, the
// chains open on it (each chain's project is the server's: ChainStanding.project) with a way to the board narrowed to it, and
// its history. Nothing here changes a project: the owner tells the Assistant, which keeps them.
const route = useRoute();
useSettle('id', () => (catalog.projects.find((p) => p.status === 'active') ?? catalog.projects[0])?.id);
const project = computed(() => catalog.projects.find((p) => p.id === route.params.id));
const archived = (p: Project) => p.status === 'archived';
// The chains on the board that are on a project; the open ones are those not finished.
const chainsOn = (id: string): Chain[] => chainRecords.value.filter((c) => standingOf(c.id).project === id);
const open = computed(() => (project.value ? chainsOn(project.value.id).filter((c) => c.status !== 'done') : []));
const sourceLabel = (id: string) => catalog.sources.find((s) => s.id === id)?.label ?? id;
</script>

<template>
  <ListPage name="projects" open :title="project?.name">
    <template #tabs><PageTabs group="work" /></template>
    <template #list>
      <NavRow v-for="p in catalog.projects" :key="p.id" :on="project?.id === p.id" :to="{ name: 'projects', params: { id: p.id } }" :description="p.customer" :data-project="p.id">
        <template #title>
          <Icon class="flex-none text-mut" name="project" :size="14" />
          <span class="min-w-0 font-medium [overflow-wrap:anywhere]" :class="{ 'text-mut': archived(p) }">{{ p.name }}</span>
          <Badge v-if="archived(p)" variant="quiet" class="flex-none" data-archived><Icon name="archive" :size="11" />Archived</Badge>
        </template>
        <span class="text-[11px] whitespace-nowrap text-mut" data-repos>{{ plural(p.repos.length, 'repository', 'repositories') }}</span>
        <span class="text-[11px] whitespace-nowrap text-mut" data-chains>{{ plural(chainsOn(p.id).length, 'chain') }} on the board</span>
      </NavRow>
    </template>
    <template v-if="project" #standing>
      <Badge v-if="archived(project)" variant="quiet" data-status="archived"><Icon name="archive" :size="11" />Archived</Badge>
      <Badge v-else variant="success" data-status="active"><Icon name="check" :size="11" />Active</Badge>
    </template>
    <EmptyPage v-if="!catalog.projects.length" title="No project yet" icon="project" say="Make a project for the storefront: it is for the shop owners who sell on it, and its code is in github.com/acme/storefront">
      <template #what>A project is one product your work is on: who it is for, your notes on it, its repositories and the connections it reads. The Chair of a chain on it reads all of that before it starts, so two products can share one Remit.</template>
      <template #how>
        <li>You tell the Assistant about the product: its name, who it is for, where its code is.</li>
        <li>It keeps the project, and each chain you start from the board narrowed to it is on it; its Chair picks one for the rest.</li>
        <li>Every chain card names its project. To change one, tell the Assistant again.</li>
      </template>
    </EmptyPage>
    <PageBody v-else-if="project">
      <div class="@container">
        <PageGrid>
          <PageSection title="About" id="project-about" data-about>
            <KeyValues>
              <dt>Customer</dt><dd data-customer :class="{ 'text-mut': !project.customer }">{{ project.customer || 'not named' }}</dd>
              <dt>Made</dt><dd><ActorTag :by="project.by" :name-of="() => 'An agent'" /> · <Tip :text="fullTime(project.createdAt)"><time class="rounded-sm" :datetime="project.createdAt"><Ago :at="project.createdAt" /></time></Tip></dd>
              <dt>Changed</dt><dd><Tip :text="fullTime(project.updatedAt)"><time class="rounded-sm" :datetime="project.updatedAt"><Ago :at="project.updatedAt" /></time></Tip></dd>
              <dt>Id</dt><dd class="mono">{{ project.id }}</dd>
            </KeyValues>
            <p v-if="archived(project)" class="mt-3 text-xs text-mut" data-archived-note>Archived: no chain is put on it any more. It stays on the objectives and chains that name it.</p>
          </PageSection>
          <PageSection title="Notes" id="project-notes" data-notes>
            <template #aside>yours, read by every Chair on it</template>
            <Markdown v-if="project.brief" :text="project.brief" />
            <EmptyState v-else>No notes yet. Tell the Assistant what the product is, where it runs and what matters in it.</EmptyState>
          </PageSection>
          <PageSection title="Repositories" :count="project.repos.length" id="project-repos" data-repos>
            <ul v-if="project.repos.length" class="m-0 flex list-none flex-col gap-1.5 p-0" aria-labelledby="project-repos">
              <li v-for="r in project.repos" :key="r" class="mono min-w-0 text-text [overflow-wrap:anywhere]" data-repo>{{ r }}</li>
            </ul>
            <EmptyState v-else>None named: a chain on it opens whichever repository its work needs.</EmptyState>
          </PageSection>
          <PageSection title="Connections" :count="project.connections.length" id="project-connections" data-connections>
            <ul v-if="project.connections.length" class="m-0 flex list-none flex-col gap-1.5 p-0" aria-labelledby="project-connections">
              <li v-for="c in project.connections" :key="`${c.source}/${c.connection}`" class="flex min-w-0 flex-wrap items-baseline gap-x-1.5" data-connection>
                <RouterLink class="text-blue-text hover:underline" :to="{ name: 'tools', params: { sourceId: c.source } }">{{ sourceLabel(c.source) }}</RouterLink>
                <span class="text-mut">connection</span><span class="mono text-text">{{ c.connection }}</span>
              </li>
            </ul>
            <EmptyState v-else>None named.</EmptyState>
          </PageSection>
          <PageSection title="Open chains" :count="open.length" id="project-chains" class="@[1100px]:col-span-2" data-open-chains>
            <template #aside><RouterLink class="text-blue-text hover:underline" :to="{ name: 'board', query: { project: project.id } }" data-board-link>The board, narrowed to it</RouterLink></template>
            <div v-if="open.length" class="grid grid-cols-1 gap-2 @[700px]:grid-cols-2">
              <ChainCard v-for="c in open" :key="c.id" :chain="c" :finished="false" :on="false" />
            </div>
            <EmptyState v-else>No chain is open on it.</EmptyState>
          </PageSection>
          <ChangeHistory class="@[1100px]:col-span-2" kind="project" :name="project.id" :rev="project.rev" />
        </PageGrid>
      </div>
    </PageBody>
    <PageBody v-else><EmptyState>This project is not listed.</EmptyState></PageBody>
  </ListPage>
</template>
