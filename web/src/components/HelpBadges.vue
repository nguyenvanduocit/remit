<script setup lang="ts">
import { computed } from 'vue';
import { helpsOf } from '../openChain';
import { HELP, mentorNamed } from '../mentors';
import Icon from './Icon.vue';
import Tip from './Tip.vue';
import { Badge } from '@/components/ui/badge';

// The mentors on card `card` whose help still stands (offered or accepted), each a badge with their name and where the help
// stands, in words beside an icon; the problem the Chair asked about is its tip. Nothing when no help is open.
const props = defineProps<{ card: string }>();
const open = computed(() => helpsOf(props.card).filter((h) => h.state === 'offered' || h.state === 'accepted'));
</script>

<template>
  <template v-for="h in open" :key="h.id">
    <Tip :text="h.problem">
      <Badge :variant="HELP[h.state].variant" class="max-w-full min-w-0" :data-help="h.id" :data-help-state="h.state">
        <Icon :name="HELP[h.state].icon" :size="11" /><span class="min-w-0 truncate">{{ mentorNamed(h.mentor)?.label ?? 'A mentor' }} · {{ HELP[h.state].label }}</span>
      </Badge>
    </Tip>
  </template>
</template>
