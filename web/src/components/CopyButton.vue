<script setup lang="ts">
import { copyText } from '../clipboard';
import Icon from './Icon.vue';
import { Button } from '@/components/ui/button';

// Copy for the text shown beside it. The text is only ever a prop, never an attribute (it can be a key); `label` names the
// button for a screen reader, as the parent's words say what is copied. `iconOnly` where a word would crowd what it sits on
// (a message in the feed).
defineProps<{ text: string; label: string; iconOnly?: boolean }>();
</script>

<template>
  <Button variant="outline" :size="iconOnly ? 'icon-xs' : 'xs'" class="flex-none" :aria-label="label" @click="copyText(text)"><Icon name="copy" :size="12" /><template v-if="!iconOnly"> Copy</template></Button>
</template>
