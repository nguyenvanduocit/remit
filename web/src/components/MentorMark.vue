<script setup lang="ts">
import { computed } from 'vue';
import { MENTOR } from '../mentors';
import Icon from './Icon.vue';

// A mentor as a square with the person icon, `size` px wide, where an agent shows its lettered square (AgentBadge). `label` is what a
// screen reader says in its place where the words beside it do not name the mentor.
const props = withDefaults(defineProps<{ size?: number; label?: string }>(), { size: 28 });
const box = computed(() => ({ width: props.size + 'px', height: props.size + 'px', borderRadius: Math.round(props.size / 4) + 'px' }));
</script>

<template>
  <div class="mark" :style="box" :role="label ? 'img' : undefined" :aria-label="label" :data-mentor-mark="MENTOR.label">
    <Icon :name="MENTOR.icon" :size="Math.round(size * 0.6)" />
  </div>
</template>

<style scoped>
.mark {
  display: flex;
  flex: none;
  align-items: center;
  justify-content: center;
  background: var(--panel3);
  color: var(--text2);
}
</style>
