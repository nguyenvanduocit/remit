<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import { sourceText } from '../library';
import type { ListedSkill } from '../core/generated/types.generated';
import { plural } from '../ui';
import CrewCard from './CrewCard.vue';
import IconChip from './IconChip.vue';
import { Badge } from '@/components/ui/badge';

// One skill on the Skills board (CrewCard): the chip of where it came from (imported in violet, written here in green), its
// name in code type, when to use it in a line, and at the end how many files it has. A click opens its page on its SKILL.md;
// the search goes along.
const props = defineProps<{ skill: ListedSkill; on: boolean }>();
const route = useRoute();
const to = computed(() => ({ name: 'skills', params: { skillName: props.skill.name }, query: { q: route.query.q } }));
</script>

<template>
  <CrewCard :to="to" :on="on" :title="skill.name" mono :data-skill="skill.name">
    <template #chip><IconChip icon="file" :class="skill.source ? 'bg-violet-bg text-violet-text' : 'bg-green-bg text-green-text'" :label="sourceText(skill.source)" /></template>
    <template #badge><Badge variant="quiet">{{ plural(skill.files.length, 'file') }}</Badge></template>
    <template #meta>{{ skill.description }}</template>
  </CrewCard>
</template>
