import type { VariantProps } from 'class-variance-authority'
import { cva } from 'class-variance-authority'

export { default as Input } from './Input.vue'

// A field stands on the control ruler (style.css --control-*) at the same heights as a button (ui/button), so a field and
// its button line up. Below md the text is 16px, which keeps a phone's browser from zooming into the field.
export const inputVariants = cva(
  'border-input focus-visible:border-ring aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive dark:aria-invalid:border-destructive/50 disabled:bg-muted border bg-background py-1 text-base transition-colors duration-(--dur-fast) file:h-6 file:text-sm file:font-medium aria-invalid:ring-3 w-full min-w-0 file:inline-flex file:border-0 file:bg-transparent file:text-foreground placeholder:text-muted-foreground disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50',
  {
    variants: {
      size: {
        default: 'h-control-md rounded-control px-control-pad-md md:text-control-md',
        sm: 'h-control-sm rounded-control-sm px-control-pad-sm md:text-control-sm',
      },
    },
    defaultVariants: {
      size: 'default',
    },
  },
)
export type InputVariants = VariantProps<typeof inputVariants>
