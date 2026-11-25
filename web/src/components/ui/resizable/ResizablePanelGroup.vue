<script setup lang="ts">
import type { SplitterGroupEmits, SplitterGroupProps } from 'reka-ui'
import type { HTMLAttributes } from 'vue'
import { reactiveOmit } from '@vueuse/core'
import { cn } from '@/lib/utils'
import { SplitterGroup, useForwardPropsEmits } from 'reka-ui'

// An arrow key moves an edge by 2% of the group, not reka's 10%: a step that is a fifth of a sidebar is too coarse to place it.
const props = withDefaults(defineProps<SplitterGroupProps & { class?: HTMLAttributes['class'] }>(), { keyboardResizeBy: 2 })
const emits = defineEmits<SplitterGroupEmits>()

const delegatedProps = reactiveOmit(props, 'class')

const forwarded = useForwardPropsEmits(delegatedProps, emits)
</script>

<template>
  <SplitterGroup
    v-slot="slotProps"
    data-slot="resizable-panel-group"
    v-bind="forwarded"
    :class="cn('flex h-full w-full data-[orientation=vertical]:flex-col', props.class)"
  >
    <slot v-bind="slotProps" />
  </SplitterGroup>
</template>
