<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import Icon from './Icon.vue';
import Kbd from './Kbd.vue';
import Spinner from './Spinner.vue';
import WorkloadPill from './WorkloadPill.vue';
import { useAssistant } from '../nav';
import { assistant } from '../assistant';
import { catalog, feedbackUnread } from '../catalog';
import type { Area } from '../router';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu';
import Tip from './Tip.vue';

// The bar over every view: where to go, what the scheduler is doing, the way to the palette (⌘K) and the way to the Assistant,
// where a chain is started. Whether
// what is shown is current is the connection banner's (ConnectionBanner.vue). The Assistant's button opens and closes its
// panel, counts what it decided for the owner since he last looked at its feed, marked while the Assistant needs the owner or failed (the tone of its standing, as the server says), and its icon turns
// into a spinner while it is working.
//
// Where to go is the five areas (router.ts Area), each an entry that opens the area's first page and is marked on every page
// of it. An entry counts what waits for the owner in its own area and nowhere else, so each count means one thing: Work the
// board's (what the server says needs him), Results the feedback not read yet. Signals, Crew and Integrations count nothing:
// an event waits on nobody. A window too narrow for the five (a phone) folds them into a menu named by the area he is in,
// with its count; a mark beside it says another area waits on him, which the menu then names.
const props = defineProps<{ toReview: number }>();
// The palette (CommandPalette, ⌘K) is the Shell's; this button opens it for the pointer.
defineEmits<{ find: [] }>();
const route = useRoute();
const { open: assisting, toggle: toggleAssistant } = useAssistant();
const assistantTone = computed(() => assistant.view?.agent.tone ?? 'idle');
const assistantCalls = computed(() => assistantTone.value === 'needs' || assistantTone.value === 'failed');
const assistantWorking = computed(() => assistantTone.value === 'running');
const assistantUnseen = computed(() => assistant.feed?.unseen ?? 0);
type Entry = { area: Area; label: string; to: string; count?: { n: number; tone: 'needs' | 'warning' | 'info' | 'quiet'; says: string } };
const entries = computed<Entry[]>(() => {
  return [
    { area: 'work', label: 'Work', to: 'board', count: catalog.loaded ? { n: props.toReview, tone: props.toReview ? 'needs' : 'quiet', says: `${props.toReview} need you` } : undefined },
    { area: 'results', label: 'Results', to: 'outcomes', count: catalog.loaded ? { n: feedbackUnread.value, tone: feedbackUnread.value ? 'needs' : 'quiet', says: `${feedbackUnread.value} unread` } : undefined },
    { area: 'signals', label: 'Signals', to: 'signals' },
    { area: 'crew', label: 'Crew', to: 'employees' },
    { area: 'integrations', label: 'Integrations', to: 'tools' },
  ];
});
const here = computed(() => entries.value.find((e) => e.area === route.meta.area));
const elsewhere = computed(() => entries.value.filter((e) => e !== here.value && e.count?.tone === 'needs'));
const say = (e: Entry) => (e.count ? `${e.label}, ${e.count.says}` : e.label);
</script>

<template>
  <header class="flex h-11 items-center gap-3.5 border-b border-line bg-bg2 px-4 max-[1535px]:gap-2.5 max-[1240px]:gap-2">
    <DropdownMenu>
      <DropdownMenuTrigger as-child>
        <Button variant="ghost" size="sm" class="areas hidden max-[640px]:inline-flex" :aria-label="['Views', here && say(here), ...elsewhere.map(say)].filter(Boolean).join('; ')" data-areas>
          {{ here?.label ?? 'Views' }}<Badge v-if="here?.count" size="count" :variant="here.count.tone">{{ here.count.n }}</Badge><i v-if="elsewhere.length" class="call" data-elsewhere></i><Icon name="chevron-down" :size="12" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" class="w-56">
        <DropdownMenuItem v-for="e in entries" :key="e.area" as-child>
          <RouterLink :to="{ name: e.to }" :aria-current="route.meta.area === e.area ? 'page' : undefined" :aria-label="e.count ? say(e) : undefined">{{ e.label }}<span class="ml-auto flex items-center gap-icon"><Badge v-if="e.count" size="count" :variant="e.count.tone">{{ e.count.n }}</Badge><Icon v-if="e === here" name="check" :size="12" /></span></RouterLink>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
    <!-- Below 641px (a phone) the five areas fold into the menu above; wider, the menu is not there. -->
    <nav class="nav flex gap-0.5 self-stretch max-[640px]:hidden" aria-label="Views">
      <RouterLink v-for="e in entries" :key="e.area" :to="{ name: e.to }" :class="{ on: route.meta.area === e.area }" :aria-current="route.meta.area === e.area ? 'page' : undefined" :aria-label="e.count ? `${e.label}, ${e.count.says}` : undefined" :data-area="e.area">
        {{ e.label }} <Badge v-if="e.count" size="count" :variant="e.count.tone">{{ e.count.n }}</Badge>
      </RouterLink>
    </nav>
    <div class="flex-1"></div>
    <WorkloadPill />
    <Tip text="Find what waits, a chain or a page, or ask the Assistant (⌘K)">
      <Button variant="outline" size="sm" aria-label="Find (⌘K)" aria-keyshortcuts="Meta+K Control+K" data-find @click="$emit('find')"><Icon name="search" :size="13" /><span class="label">Find</span><Kbd class="label">⌘K</Kbd></Button>
    </Tip>
    <Tip text="Talk to the Assistant about anything in Remit, from any page">
      <Button variant="outline" size="sm" class="relative" :aria-pressed="assisting" :aria-label="[assistantCalls || assistantWorking ? `Assistant: ${assistant.view?.agent.label}` : 'Assistant', assistantUnseen ? `${assistantUnseen} decided for you since you looked` : ''].filter(Boolean).join(', ')" data-assistant-toggle @click="toggleAssistant()">
        <Spinner v-if="assistantWorking" :size="14" data-working /><Icon v-else name="spark" :size="13" /><span class="label">Assistant</span><Badge v-if="assistantUnseen" size="count" variant="info" data-assistant-unseen>{{ assistantUnseen }}</Badge><i v-if="assistantCalls" class="call" :class="assistantTone" data-calls></i>
      </Button>
    </Tip>
  </header>
</template>

<style scoped>
.nav a {
  position: relative;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 11px;
  color: var(--mut);
  text-decoration: none;
}
.nav a:hover {
  color: var(--text);
}
/* Below 1711px the buttons keep only their icons (their tips say the rest), below 1535px the links tighten, below 1240px (a
   1024px screen) they tighten again and below 1100px once more. WorkloadPill.vue says less at 1535px and 1240px. */
@media (max-width: 1710px) {
  .label {
    display: none;
  }
}
@media (max-width: 1535px) {
  .nav a {
    padding: 6px 9px;
  }
}
@media (max-width: 1240px) {
  .nav a {
    padding: 6px 7px;
  }
}
@media (max-width: 1100px) {
  .nav a {
    padding: 6px 5px;
  }
}
/* The Assistant needs the owner, or failed: a dot at its button's corner (open, the button looks pressed: aria-pressed), in the
   orange of everything that waits on him (the `needs` tone), red when it failed. */
.call {
  position: absolute;
  top: -3px;
  right: -3px;
  width: 8px;
  height: 8px;
  border: 1.5px solid var(--bg2);
  border-radius: 50%;
  background: var(--orange);
}
.call.failed {
  background: var(--red);
}
/* Another area waits on the owner: the same orange mark, beside the folded menu's name rather than on a corner. */
.areas .call {
  position: static;
  border: 0;
}
/* The area the owner is in: its label in full text colour over a bar on the bar's bottom edge, the way a tab is marked. */
.nav a.on {
  color: var(--text);
}
.nav a.on::after {
  content: '';
  position: absolute;
  right: 4px;
  bottom: -1px;
  left: 4px;
  height: 2px;
  border-radius: 2px 2px 0 0;
  background: var(--text);
}
</style>
