<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { useRoute } from 'vue-router';
import { useQueryRef } from '../nav';
import { useHeadingTag } from '../pageHeading';
import { openRepo, openedRepo } from '../releases';
import EmptyPage from './EmptyPage.vue';
import EmptyState from './EmptyState.vue';
import ListState from './ListState.vue';
import PageTabs from './PageTabs.vue';
import ReleaseCard from './ReleaseCard.vue';
import ReleaseDetail from './ReleaseDetail.vue';
import ReleasePauseNote from './ReleasePauseNote.vue';
import ShowMore from './ShowMore.vue';

// A repository's releases (ADR-0116), live from the core as the server lays the repository out (RepoView): ?repo= names
// the repository (its catalog id, as its queue's landings name it), its last release pause above, a card per release of
// the newest it keeps, newest first, then the one picked (?release=, else the newest) as its record. Releasing, baking, taking back and holding are code and the Assistant's: the owner
// tells the Assistant to hold or take back, and reads here how it went.
const route = useRoute();
const repo = useQueryRef<string>('repo', '');
watch(repo, (r) => openRepo(r), { immediate: true });
onBeforeUnmount(() => openRepo(''));

// The view of the repository the address names: until it is read, the one before may still be held.
const view = computed(() => (openedRepo.value?.repo === repo.value ? openedRepo.value : null));
const all = computed(() => view.value?.releases ?? []);
const picked = useQueryRef<string>('release', '');
const current = computed(() => all.value.find((r) => r.release.id === picked.value) ?? all.value[0]);
// The cards a page at a time (a repository releases without bound), the open one always among them.
const PAGE = 12;
const shown = ref(PAGE);
const cards = computed(() => {
  const page = all.value.slice(0, shown.value);
  return current.value && !page.includes(current.value) ? [...page, current.value] : page;
});
const toRelease = (id: string) => ({ query: { ...route.query, release: id } });
const Heading = useHeadingTag();
</script>

<template>
  <section class="flex min-h-0 flex-1 flex-col overflow-hidden" :aria-label="repo ? `Releases of ${repo}` : 'Releases'">
    <PageTabs group="work" />
    <EmptyPage v-if="!repo" title="Which repository’s releases?" icon="release">
      <template #what>Each repository releases what lands on its base by its own procedure, and Remit takes a release back when a guardrail breaches while it bakes.</template>
      <template #how>
        <li>Open a merge queue and follow its Releases link, or a landing’s release on its chain’s screen.</li>
        <li>To hold a repository’s releases or take one back, tell the Assistant.</li>
      </template>
    </EmptyPage>
    <div v-else class="flex min-h-0 flex-1 flex-col gap-5 overflow-auto px-pane pt-3.5 pb-5">
      <component :is="Heading" class="flex items-baseline gap-2 text-[15px] font-semibold text-text">Releases <span class="mono text-[13px] font-normal text-text2">{{ repo }}</span></component>
      <ReleasePauseNote v-if="view?.pause" :pause="view.pause" />
      <ListState v-if="!view" what="the releases" :held="false" />
      <EmptyState v-else-if="!current">Nothing released yet: once a change lands on its base, the repository’s procedure releases it and it shows here.</EmptyState>
      <template v-else>
        <div v-if="all.length > 1">
          <ul class="m-0 grid list-none gap-2.5 p-0 [grid-template-columns:repeat(auto-fill,minmax(260px,1fr))]" :aria-label="`Releases of ${repo}`" data-releases>
            <li v-for="{ release: r } in cards" :key="r.id" class="min-w-0"><ReleaseCard :release="r" :to="toRelease(r.id)" :on="r.id === current.release.id" /></li>
          </ul>
          <ShowMore v-if="all.length > shown" class="mt-2.5" @click="shown += PAGE">Show {{ Math.min(PAGE, all.length - shown) }} more of {{ all.length - shown }} releases</ShowMore>
        </div>
        <ReleaseDetail :release="current.release" :carried="current.landings" />
      </template>
    </div>
  </section>
</template>
