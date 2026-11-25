<script setup lang="ts">
import { computed } from 'vue';
import { catalog, projectOf } from '../catalog';
import Icon from './Icon.vue';
import { Button } from '@/components/ui/button';
import { DropdownMenu, DropdownMenuContent, DropdownMenuLabel, DropdownMenuRadioGroup, DropdownMenuRadioItem, DropdownMenuSeparator, DropdownMenuTrigger } from '@/components/ui/dropdown-menu';

// What narrows a view of the work to one project (ADR-0108): the board, the column beside an open chain, the Objectives tab. A
// menu of every project, or all of them; the button names the one picked while one is. An archived project is offered to no
// one, so it is listed only while the view is narrowed to it (a link named it). With no project defined and none named, nothing shows.
// `project` is the id (?project=, nav.ts useProjectRef), '' for all; `size` is its step on the control ruler, as the search's beside it.
withDefaults(defineProps<{ size?: 'default' | 'sm' }>(), { size: 'sm' });
const project = defineModel<string>({ required: true });
const offered = computed(() => catalog.projects.filter((p) => p.status === 'active' || p.id === project.value));
const picked = computed(() => projectOf(project.value));
// A link may name an id the server does not list: it shows as that id.
const name = computed(() => picked.value?.name ?? project.value);
const pick = (v: unknown) => (project.value = v === '*' ? '' : String(v));
</script>

<template>
  <DropdownMenu v-if="catalog.projects.length || project">
    <DropdownMenuTrigger as-child>
      <Button class="flex-none" :variant="project ? 'secondary' : 'outline'" :size="size" :aria-label="`Project: ${project ? name : 'all'}`" data-project-filter>
        <Icon name="project" :size="13" /><span v-if="project" class="max-w-32 truncate">{{ name }}</span>
      </Button>
    </DropdownMenuTrigger>
    <DropdownMenuContent align="end" class="w-72">
      <DropdownMenuLabel>Project</DropdownMenuLabel>
      <DropdownMenuRadioGroup :model-value="project || '*'" @update:model-value="pick">
        <DropdownMenuRadioItem value="*">All projects</DropdownMenuRadioItem>
        <DropdownMenuSeparator />
        <DropdownMenuRadioItem v-for="p in offered" :key="p.id" :value="p.id" :data-project="p.id">
          <span class="min-w-0 [overflow-wrap:anywhere]">{{ p.name }}</span><span v-if="p.status === 'archived'" class="ml-auto text-muted-foreground">archived</span>
        </DropdownMenuRadioItem>
        <DropdownMenuRadioItem v-if="project && !picked" :value="project">
          <span class="min-w-0 truncate">{{ project }}</span><span class="ml-auto text-muted-foreground">not listed</span>
        </DropdownMenuRadioItem>
      </DropdownMenuRadioGroup>
    </DropdownMenuContent>
  </DropdownMenu>
</template>
