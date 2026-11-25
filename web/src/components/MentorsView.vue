<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import { useApi } from '../core/api';
import { catalog, standingOfMentor } from '../catalog';
import { MENTOR, WORKER_KEY_MASK } from '../mentors';
import type { Mentor } from '../core/generated/types.generated';
import { fullTime, plural } from '../ui';
import { useRemote } from '../useRemote';
import Ago from './Ago.vue';
import EmptyPage from './EmptyPage.vue';
import EmptyState from './EmptyState.vue';
import Icon from './Icon.vue';
import KeyReveal from './KeyReveal.vue';
import KeyValues from './KeyValues.vue';
import ListPage from './ListPage.vue';
import MemoryBook from './MemoryBook.vue';
import MentorMark from './MentorMark.vue';
import MentorPresence from './MentorPresence.vue';
import NavRow from './NavRow.vue';
import PageBody from './PageBody.vue';
import PageGrid from './PageGrid.vue';
import PageSection from './PageSection.vue';
import PageTabs from './PageTabs.vue';
import Tip from './Tip.vue';
import { Badge } from '@/components/ui/badge';

// The people who help the AI employees (ADR-0062), in the order the server lists them: each says what they are strong in, when
// they said they can help, where they stand with their apps and how many helps of theirs are open, all in the server's words.
// The open one (/mentors/<id>, else the first) adds the codes they work from, each shown to the owner on his asking (KeyReveal)
// so he passes it on, and what they taught, as their own memory keeps it (`mentor_memory`), read as they open: no push carries a
// memory. Nothing here changes a mentor: the owner tells the Assistant to add one, make them a code or remove them.
const route = useRoute();
const api = useApi();
const mentor = computed(() => catalog.mentors.find((m) => m.id === route.params.id) ?? catalog.mentors[0]);
const standing = computed(() => (mentor.value ? standingOfMentor(mentor.value.id) : undefined));
const codes = computed(() => standing.value?.keys ?? []);
// The address a mentor signs in at with a code: this app's, carrying the code (`#key=`, taken out of the address on load).
const signInLink = (code: string) => `${location.origin}/#key=${encodeURIComponent(code)}`;
const openHelps = (m: Mentor) => standingOfMentor(m.id)?.openHelps ?? 0;
const removed = (m: Mentor) => m.status === 'removed';
const memory = useRemote(
  () => (mentor.value ? { id: mentor.value.id } : undefined),
  (asked) => api.mentorMemory(asked.id),
);
const taught = computed(() => (memory.of.value?.id === mentor.value?.id ? memory.data.value : undefined));
</script>

<template>
  <ListPage name="mentors" open :title="mentor?.label">
    <template #tabs><PageTabs group="crew" /></template>
    <template #list>
      <NavRow v-for="m in catalog.mentors" :key="m.id" :on="mentor?.id === m.id" :to="{ name: 'mentors', params: { id: m.id } }" :description="m.strengths" :data-mentor="m.id">
        <template #title>
          <MentorMark :size="18" />
          <span class="min-w-0 font-medium [overflow-wrap:anywhere]" :class="{ 'text-mut': removed(m) }">{{ m.label }}</span>
          <Badge v-if="removed(m)" variant="quiet" class="flex-none" data-removed><Icon name="archive" :size="11" />Removed</Badge>
        </template>
        <MentorPresence v-if="!removed(m)" :mentor="m.id" />
        <Badge v-if="openHelps(m)" variant="info" class="flex-none" data-open-helps><Icon name="person" :size="11" />{{ plural(openHelps(m), 'open help') }}</Badge>
        <span v-if="m.availability && !removed(m)" class="min-w-0 truncate text-[11px] text-mut" data-availability>{{ m.availability.note }}</span>
      </NavRow>
    </template>
    <template v-if="mentor" #standing>
      <Badge variant="outline" data-kind><Icon :name="MENTOR.icon" :size="11" />{{ MENTOR.label }}</Badge>
      <Badge v-if="removed(mentor)" variant="quiet" data-status="removed"><Icon name="archive" :size="11" />Removed</Badge>
      <MentorPresence v-else :mentor="mentor.id" />
    </template>
    <EmptyPage v-if="!mentor" title="No mentor yet" icon="person" say="Add Lan as a mentor, strong in visual design">
      <template #what>A mentor is a person who helps your AI employees. When a card is stuck on something a person knows better, its Chair asks a mentor whose strengths fit; once they accept, their messages steer that card's session.</template>
      <template #how>
        <li>Tell the Assistant who to add and what they are strong in; then have it make them a code for each device they work from.</li>
        <li>Show the code here and copy its sign-in link to pass on: they open it, or paste the code on Remit’s sign-in page, and answer the helps they are asked for on their own board.</li>
        <li>A card a mentor was asked to help, or helps, says so with their name on the chain's tree and in its drawer.</li>
      </template>
    </EmptyPage>
    <PageBody v-else>
      <div class="@container">
        <PageGrid>
          <PageSection title="About" id="mentor-about" data-about>
            <KeyValues>
              <dt>Strong in</dt><dd data-strengths>{{ mentor.strengths || 'not said yet' }}</dd>
              <dt>Can help</dt>
              <dd v-if="mentor.availability" data-availability>{{ mentor.availability.note }} · <span class="text-mut">said <Tip :text="fullTime(mentor.availability.at)"><time class="rounded-sm" :datetime="mentor.availability.at"><Ago :at="mentor.availability.at" /></time></Tip></span></dd>
              <dd v-else class="text-mut" data-availability>not stated</dd>
              <dt>Where they are</dt><dd data-presence>{{ standing?.presence ?? '—' }}</dd>
              <dt>Open helps</dt><dd data-open-helps>{{ standing ? plural(standing.openHelps, 'help') : '—' }}</dd>
              <dt>Added</dt><dd><Tip :text="fullTime(mentor.createdAt)"><time class="rounded-sm" :datetime="mentor.createdAt"><Ago :at="mentor.createdAt" /></time></Tip></dd>
            </KeyValues>
            <p v-if="removed(mentor)" class="mt-3 text-xs text-mut" data-removed-note>Removed: they are asked for no help, and their codes were revoked. Their past helps stay on the cards they helped.</p>
          </PageSection>
          <PageSection v-if="!removed(mentor)" title="Codes" id="mentor-codes" :count="codes.length" data-codes>
            <template #aside>one per device they work from</template>
            <ul v-if="codes.length" class="m-0 flex list-none flex-col gap-2.5 p-0" :aria-label="`The codes ${mentor.label} works from`">
              <li v-for="c in codes" :key="c.id" class="flex flex-col gap-2 rounded-lg bg-bg2 px-3 py-2.5" :data-code="c.id">
                <div class="flex min-w-0 flex-wrap items-baseline gap-x-1.5 gap-y-1 text-xs">
                  <Tip :text="c.note || c.id" when-clipped><span class="min-w-0 max-w-[36ch] truncate font-medium text-text" data-code-note>{{ c.note || 'no note' }}</span></Tip>
                  <span :class="c.open ? 'text-text' : 'text-mut'" data-code-state>{{ c.state }}</span>
                  <span class="text-mut">· made <Tip :text="fullTime(c.createdAt)"><time class="rounded-sm" :datetime="c.createdAt"><Ago :at="c.createdAt" /></time></Tip></span>
                  <span class="text-mut" data-code-used>· <template v-if="c.lastUsedAt">last used <Tip :text="fullTime(c.lastUsedAt)"><time class="rounded-sm" :datetime="c.lastUsedAt"><Ago :at="c.lastUsedAt" /></time></Tip></template><template v-else>not used yet</template></span>
                </div>
                <KeyReveal :key="c.id" noun="code" :name="c.note || c.id" :mask="WORKER_KEY_MASK" :read="() => api.workerKey(c.id)" :link="signInLink" />
              </li>
            </ul>
            <EmptyState v-else data-codes-empty>None yet. Tell the Assistant to make {{ mentor.label }} a code for each device they work from, then show it here and pass its sign-in link on.</EmptyState>
          </PageSection>
          <PageSection title="What they taught" id="mentor-memory" data-memory>
            <template #aside>learned from their messages to the agents they help</template>
            <MemoryBook :book="taught" :error="memory.error.value" :loading="memory.loading.value && memory.of.value?.id !== mentor.id" :who="mentor.label" view-key="memory" surface="panel" :teacher="mentor.label" :retry="memory.retry" />
          </PageSection>
        </PageGrid>
      </div>
    </PageBody>
  </ListPage>
</template>
