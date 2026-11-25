<script setup lang="ts">
import { computed } from 'vue';
import { agentClass, agentLetter, agentStyle } from '../ui';

// An agent as a lettered square, `size` px wide (rounded in proportion): `agent` is the id of its Agent. `label` is what a
// screen reader says in its place where the words beside it do not name it (a card's chip).
const props = withDefaults(defineProps<{ agent: string; size?: number; label?: string }>(), { size: 28 });
const box = computed(() => ({ width: props.size + 'px', height: props.size + 'px', borderRadius: Math.round(props.size / 4) + 'px', fontSize: (props.size < 26 ? 10 : 11.5) + 'px' }));
</script>

<template>
  <div class="badge" :class="agentClass(agent)" :style="{ ...agentStyle(agent), ...box }" :role="label ? 'img' : undefined" :aria-label="label">{{ agentLetter(agent) }}</div>
</template>

<style scoped>
/* An agent's letter on its colour: known agents have theirs here, any other gets one from its name (agentStyle). */
.badge {
  display: flex;
  flex: none;
  align-items: center;
  justify-content: center;
  font-weight: 700;
}
.A-research { background: var(--agent-research-bg); color: var(--cyan); }
.A-architect { background: var(--agent-architect-bg); color: var(--blue); }
.A-engineer { background: var(--agent-engineer-bg); color: var(--green); }
.A-reviewer { background: var(--agent-reviewer-bg); color: var(--amber); }
.A-qa, .A-qc { background: var(--agent-qa-bg); color: var(--violet); }
.A-pm { background: var(--agent-pm-bg); color: var(--agent-pm); }
.A-chair { background: var(--agent-chair-bg); color: var(--agent-chair); }
.A-critic { background: var(--agent-critic-bg); color: var(--agent-critic); }
.A-security { background: var(--agent-security-bg); color: var(--agent-security); }
.A-release { background: var(--agent-release-bg); color: var(--pink); }
.A-human { background: var(--agent-human-bg); color: var(--sand); }
.A-judge { background: var(--agent-judge-bg); color: var(--sand); }
.A-scout { background: var(--agent-scout-bg); color: var(--blue-hi); }
</style>
