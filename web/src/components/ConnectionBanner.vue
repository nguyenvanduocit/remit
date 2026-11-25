<script setup lang="ts">
import { computed } from 'vue';
import { said } from '../banner';
import { session } from '../session';
import { ask } from '../replica';
import { Button } from '@/components/ui/button';
import Icon from './Icon.vue';

// Under the top bar whenever what is shown may not be current (ADR-0094), saying what the core says of the link: the data
// is loading (syncing) or could not be loaded (stale), or the link is down, when the page stays as it was, read-only, and
// the banner says since when, when it tries again (with Try now), and how many of the owner's messages wait to go out. A
// page of another build than the server's says to reload. Once the link is live again it says so for a moment, then goes.
// A screen reader hears each change of state once, without the countdown (the polite live region). In a narrow column the
// words wrap rather than clip, so what the banner says is never cut.
const banner = computed(() => session.banner);
const text = computed(() => (banner.value ? said(banner.value) : ''));
const tone = computed(() => banner.value?.tone);
const tryNow = () => ask({ ask: 'connect' });
const reload = () => location.reload();
</script>

<template>
  <div
    v-if="banner"
    class="flex min-h-9 shrink-0 items-center gap-2 border-b px-4 py-1.5 text-[12.5px]"
    :class="tone === 'back' ? 'border-green-line bg-green-bg text-green-text' : tone === 'loading' ? 'border-blue-line bg-blue-bg text-blue-hi' : 'border-amber-line bg-amber-bg text-amber'"
    :data-phase="tone === 'back' ? 'back' : session.link.phase"
  >
    <template v-if="tone === 'back'"><Icon name="check" :size="12" />{{ text }}</template>
    <template v-else>
      <Icon v-if="tone === 'down'" name="alert" :size="12" class="flex-none" />
      <span class="min-w-0" data-banner-text>{{ text }}<template v-if="banner.waits">{{ ' ' }}<span data-queued>{{ banner.waits }}</span></template></span>
      <Button v-if="banner.action === 'reload'" size="xs" variant="outline" class="ml-auto flex-none" data-reload @click="reload"><Icon name="repeat" :size="11" />Reload</Button>
      <Button v-else-if="banner.action === 'tryNow'" size="xs" variant="outline" class="ml-auto flex-none" data-try-now @click="tryNow"><Icon name="repeat" :size="11" />Try now</Button>
    </template>
  </div>
  <div role="status" aria-live="polite" class="sr-only" data-announce>{{ banner?.announce }}</div>
</template>
