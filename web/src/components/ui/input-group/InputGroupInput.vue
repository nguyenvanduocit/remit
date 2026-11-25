<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { cn } from '@/lib/utils'
import type { InputVariants } from '@/components/ui/input'
import { Input } from '@/components/ui/input'

const props = defineProps<{
  size?: InputVariants['size']
  class?: HTMLAttributes['class']
}>()
</script>

<template>
  <Input
    data-slot="input-group-control"
    :size="size"
    :class="cn(
      'rounded-none border-0 bg-transparent shadow-none outline-none ring-0 focus-visible:ring-0 disabled:bg-transparent aria-invalid:ring-0 dark:bg-transparent dark:disabled:bg-transparent flex-1',
      props.class,
    )"
  />
</template>
