import type { VariantProps } from 'class-variance-authority'
import { cva } from 'class-variance-authority'

export { default as Badge } from './Badge.vue'

export const badgeVariants = cva(
  'gap-[5px] rounded-4xl border border-transparent py-0.5 text-[11px] font-normal transition-[color,background-color,border-color,box-shadow] has-data-[icon=inline-end]:pr-1.5 has-data-[icon=inline-start]:pl-1.5 [&>svg]:size-3! group/badge inline-flex w-fit shrink-0 items-center justify-center overflow-hidden whitespace-nowrap [button&]:overflow-visible [button&]:hit focus-visible:border-ring aria-invalid:border-destructive aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 [&>svg]:pointer-events-none',
  {
    variants: {
      variant: {
        default: 'bg-primary text-primary-foreground [a]:hover:bg-primary/80',
        secondary: 'bg-secondary text-secondary-foreground [a]:hover:bg-secondary/80',
        destructive: 'bg-destructive/10 [a]:hover:bg-destructive/20 text-destructive dark:bg-destructive/20',
        outline: 'border-border text-foreground [a]:hover:bg-muted [a]:hover:text-muted-foreground',
        ghost: 'hover:bg-muted hover:text-muted-foreground dark:hover:bg-muted/50',
        link: 'text-primary underline-offset-4 hover:underline',
        // Where something stands (statusTone in ui.ts picks one): an outline in the colour of its state. One that opens more
        // (a button) lights up under the pointer.
        quiet: 'border-border bg-secondary text-muted-foreground [button]:hover:bg-muted [button]:hover:text-foreground',
        info: 'border-info/30 bg-info/10 text-info',
        success: 'border-success/30 bg-success/10 text-success [a]:hover:border-success',
        warning: 'border-warning/30 bg-warning/10 text-warning',
        gate: 'border-sand/30 bg-sand/10 text-sand [button]:hover:bg-sand/20',
        danger: 'border-destructive/30 bg-destructive/10 text-destructive',
        // an agent something waits on the owner with (standing's `needs` tone), as its drawer's header and its lane mark it
        needs: 'border-orange/30 bg-orange-tint text-orange-text',
        // Who acted (ActorTag), where it matters that it was not an agent: the owner in ink, as people are, the Assistant in
        // its violet (ADR-0090).
        owner: 'border-line2 bg-panel3 text-text',
        assistant: 'border-violet-line bg-violet-bg text-violet-text',
      },
      size: {
        default: 'h-5 px-2',
        // a count beside a label (the nav's "Board 3")
        count: 'h-[18px] min-w-[18px] px-[5px]',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  },
)
export type BadgeVariants = VariantProps<typeof badgeVariants>
