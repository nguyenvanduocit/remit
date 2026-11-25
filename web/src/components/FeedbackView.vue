<script setup lang="ts">
import { computed, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { attempt } from '../act';
import { useApi } from '../core/api';
import { catalog, FEEDBACK_KEPT, listedChain } from '../catalog';
import { feedbackKindOf, useFeedbackColumns } from '../feedbackColumns';
import type { Feedback } from '../core/generated/types.generated';
import { modelEffort } from '../ui';
import Ago from './Ago.vue';
import EmptyPage from './EmptyPage.vue';
import FeedbackColumn from './FeedbackColumn.vue';
import FeedbackKindIcon from './FeedbackKindIcon.vue';
import FeedbackSidebar from './FeedbackSidebar.vue';
import KeyValues from './KeyValues.vue';
import ListPage from './ListPage.vue';
import Markdown from './Markdown.vue';
import PageBody from './PageBody.vue';
import PageSection from './PageSection.vue';
import PageTabs from './PageTabs.vue';
import SearchBox from './SearchBox.vue';
import { Badge } from '@/components/ui/badge';
import PageGrid from './PageGrid.vue';

// The mailbox of the organisation: what agents told it about how they work (catalog.feedback, the server's newest 200), each in
// the agent's own words, in the board's shape. With nothing open, the page is the board: the search, then
// every piece of feedback as a card in one column per kind, newest first, a page at a time. A card opens one: the
// address names it, the board folds to its column beside it (FeedbackSidebar, ?col=), and the page says what the agent wrote,
// who it was and where it worked. Opening one reads it, which the server records (feedbackRead) and the top bar's count follows.
// Nothing here acts on it: the owner tells an agent what to change. With no feedback yet, the page says how it comes (EmptyPage).
const route = useRoute();
const router = useRouter();
const api = useApi();
const { columns, query, more } = useFeedbackColumns();
const id = computed(() => (typeof route.params.id === 'string' ? route.params.id : ''));
const item = computed(() => catalog.feedback.find((f) => f.id === id.value));
// A chain's feedback links to the chain, at the agent that sent it; the Assistant's home is no chain.
const chainOf = (f: Feedback) => (f.home.startsWith('ch_') ? { name: 'chain', params: { id: f.home }, query: { card: f.cardId } } : undefined);
const chainTitle = (f: Feedback) => listedChain(f.home)?.title ?? f.home;
watch(
  () => (item.value && !item.value.readAt ? item.value.id : ''),
  (fid) => {
    if (fid) void attempt(() => api.feedbackRead(fid));
  },
  { immediate: true },
);

// Esc on an open one's page goes back to the whole board; in a search with words in it, Esc only clears them (SearchBox), so
// the page looks at the key before the search does (capture), while the words are still there.
const back = () => void router.push({ name: 'feedback', query: { q: route.query.q } });
const onEsc = (e: KeyboardEvent) => {
  if (e.target instanceof HTMLInputElement && e.target.value) return;
  back();
};
</script>

<template>
  <section v-if="!id" class="flex min-h-0 flex-1 flex-col overflow-hidden" data-board>
    <PageTabs group="results"><template v-if="catalog.feedback.length" #end><SearchBox v-model="query" size="sm" class="w-[320px] max-w-full" placeholder="Search feedback…" label="Search feedback" /></template></PageTabs>
    <div v-if="!catalog.feedback.length" class="flex min-h-0 min-w-0 flex-1 flex-col">
    <EmptyPage title="No feedback yet" icon="comment" say="Ask every agent of the chain what got in its way, and have each send it as feedback">
      <template #what>Every agent has a tool to tell the organisation how it works: what it was asked to do and how (How I work), what got in its way (Difficulty), what it complains of (Complaint) and what it would change (Suggestion). What they send lands here, in their own words, for you to read; nothing acts on it by itself.</template>
      <template #how>
        <li>An agent sends feedback on its own when something gets in its way, as its brief asks it to.</li>
        <li>Each piece is listed here under its kind, newest first, unread until you open it.</li>
        <li>You read it and tell the agent, its Chair or the Assistant what to change in a brief or a playbook.</li>
      </template>
    </EmptyPage>
    </div>
    <template v-else>
      <div class="grid min-h-0 flex-1 auto-cols-[minmax(240px,1fr)] grid-flow-col gap-2.5 overflow-x-auto px-4 pt-2.5 pb-3.5">
        <FeedbackColumn v-for="col in columns" :key="col.key" class="rounded-lg bg-bg2 p-2" :column="col" @more="more(col.key)" />
      </div>
      <p v-if="catalog.feedback.length >= FEEDBACK_KEPT" class="flex-none px-4 pb-2 text-center text-xs text-faint" data-kept>The newest {{ FEEDBACK_KEPT }} are listed.</p>
    </template>
  </section>
  <ListPage v-else name="feedback" :open="!!item" :title="item?.name" empty="This feedback is not listed any more." @keydown.esc.capture="onEsc">
    <template #tabs><PageTabs group="results" /></template>
    <template #sidebar><FeedbackSidebar /></template>
    <template v-if="item" #standing><Badge variant="quiet" data-kind-badge>{{ feedbackKindOf(item.kind)?.label }}</Badge></template>
    <PageBody v-if="item">
      <!-- The cards fill the page; wide enough (two cards of a readable width), they stand two abreast, each its own height;
           what is wide in itself (a flow, a guide with its command) takes the whole row. -->
      <div class="@container">
      <PageGrid>
        <PageSection :title="`What ${item.name} says`" id="feedback-text" data-said>
          <template #aside><FeedbackKindIcon :kind="item.kind" labelled /></template>
          <Markdown :text="item.text" type="body" data-text />
        </PageSection>
        <PageSection title="Who sent it" id="feedback-from" data-about>
          <KeyValues>
            <dt>From</dt>
            <dd data-from><RouterLink v-if="chainOf(item)" :to="chainOf(item)!" class="text-blue-text hover:underline">{{ item.name }}</RouterLink><template v-else>{{ item.name }}</template> · {{ item.agent }} on {{ modelEffort(item.model, item.effort) }}</dd>
            <template v-if="chainOf(item)"><dt>Chain</dt><dd><RouterLink :to="chainOf(item)!" class="text-blue-text hover:underline" data-chain>{{ chainTitle(item) }}</RouterLink></dd></template>
            <dt>Sent</dt><dd data-sent><Ago :at="item.createdAt" /></dd>
            <dt>Read</dt><dd data-read><template v-if="item.readAt"><Ago :at="item.readAt" /></template><template v-else>Just now</template></dd>
          </KeyValues>
        </PageSection>
      </PageGrid>
      </div>
    </PageBody>
  </ListPage>
</template>
