<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { attempt, useAct } from '../act';
import { useApi } from '../core/api';
import type { AgentStanding, AssistantRuntimeOptions, Card } from '../core/generated/types.generated';
import { session } from '../session';
import Icon from './Icon.vue';
import { Button } from '@/components/ui/button';
import { DropdownMenu, DropdownMenuContent, DropdownMenuLabel, DropdownMenuRadioGroup, DropdownMenuRadioItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu';

const props = defineProps<{ card: Card; agent: AgentStanding }>();
const api = useApi();
const act = useAct();
const options = ref<AssistantRuntimeOptions>();
const saving = ref(false);
const blocked = computed(() => session.blocked ?? (props.agent.asks.includes('change_runtime') ? undefined : 'Available after the current turn'));

const fields = [
  { key: 'model', label: 'Model', choices: 'models', defaultLabel: 'Default model' },
  { key: 'effort', label: 'Thinking effort', choices: 'efforts', defaultLabel: 'Default thinking' },
] as const;

watch(() => session.blocked, async (down, _, onCleanup) => {
  if (down) return;
  let current = true;
  onCleanup(() => { current = false; });
  const answer = await attempt(() => api.assistantRuntimeOptions());
  if (current) options.value = answer;
}, { immediate: true });

async function change(field: 'model' | 'effort', value: unknown) {
  if (typeof value !== 'string' || value === props.card[field] || blocked.value || saving.value) return;
  saving.value = true;
  try {
    await act('assistantSetRuntime', [field === 'model' ? value : null, field === 'effort' ? value : null]);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <div v-if="options" class="flex min-w-0 flex-wrap items-center gap-1.5" :title="blocked" :aria-busy="saving">
    <DropdownMenu v-for="field in fields" :key="field.key">
      <DropdownMenuTrigger as-child>
        <Button
          variant="ghost"
          size="xs"
          :aria-label="`Assistant ${field.label.toLowerCase()}, ${card[field.key] || field.defaultLabel}`"
          :disabled="!!blocked || (field.key === 'model' && !options.models.length)"
          :aria-disabled="saving || undefined"
          class="text-mut hover:text-text aria-disabled:opacity-50"
          :data-assistant-runtime="field.key"
        >
          <span class="max-w-28 truncate">{{ card[field.key] || field.defaultLabel }}</span>
          <Icon name="chevron-down" :size="12" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" class="min-w-36">
        <DropdownMenuLabel>{{ field.label }}</DropdownMenuLabel>
        <DropdownMenuRadioGroup :model-value="card[field.key]" @update:model-value="change(field.key, $event)">
          <DropdownMenuRadioItem v-if="field.key === 'effort'" value="">{{ field.defaultLabel }}</DropdownMenuRadioItem>
          <DropdownMenuRadioItem v-for="choice in options[field.choices]" :key="choice" :value="choice">{{ choice }}</DropdownMenuRadioItem>
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  </div>
</template>
