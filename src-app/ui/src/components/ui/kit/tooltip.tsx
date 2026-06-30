import * as React from 'react'
import { Slot } from '@radix-ui/react-slot'
import { Tooltip as TT, TooltipTrigger, TooltipContent, TooltipProvider } from '../shadcn/tooltip'

export interface TooltipProps {
  /** Tooltip body. `title` is an accepted alias (legacy uses `title`). */
  content?: React.ReactNode
  title?: React.ReactNode
  side?: 'top' | 'right' | 'bottom' | 'left'
  delay?: number
  className?: string
  children: React.ReactElement
  /** Any other props (and the ref) are forwarded onto the child element. This
   *  lets <Tooltip> sit directly inside an `asChild` slot (DropdownMenuTrigger,
   *  PopoverTrigger, …): the parent injects onClick/ref onto <Tooltip>, which a
   *  plain function component would silently drop — breaking the trigger. Slot
   *  merges them onto the child instead. */
  [key: string]: unknown
}

export const Tooltip = React.forwardRef<HTMLElement, TooltipProps>(function Tooltip(
  { content, title, side = 'top', delay = 300, className, children, ...rest },
  ref,
) {
  const body = content ?? title
  // Merge any parent-injected props (e.g. an asChild trigger's onClick/ref)
  // onto the child so composition works regardless of nesting order.
  const child = (
    <Slot ref={ref as React.Ref<HTMLElement>} {...rest}>
      {children}
    </Slot>
  )
  if (body == null) return child
  return (
    <TooltipProvider delayDuration={delay}>
      <TT>
        <TooltipTrigger asChild>{child}</TooltipTrigger>
        <TooltipContent side={side} className={className}>{body}</TooltipContent>
      </TT>
    </TooltipProvider>
  )
})
