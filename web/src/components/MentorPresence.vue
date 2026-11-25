<script setup lang="ts">
import { computed } from 'vue';
import { standingOfMentor } from '../catalog';
import { presenceLook } from '../mentors';
import Icon from './Icon.vue';
import { Badge } from '@/components/ui/badge';

// Where mentor `mentor` stands with their apps, in the server's words; nothing until the server has stood them.
const props = defineProps<{ mentor: string }>();
const look = computed(() => {
  const p = standingOfMentor(props.mentor);
  return p && presenceLook(p);
});
</script>

<template>
  <Badge v-if="look" :variant="look.variant" class="flex-none" :data-presence="look.label"><Icon :name="look.icon" :size="11" />{{ look.label }}</Badge>
</template>
