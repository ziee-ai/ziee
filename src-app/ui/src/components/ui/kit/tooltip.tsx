import * as React from 'react'
import { Tooltip as TT, TooltipTrigger, TooltipContent, TooltipProvider } from '../shadcn/tooltip'

export interface TooltipProps {
  /** Tooltip body. `title` is an accepted alias (legacy uses `title`). */
  content?: React.ReactNode
  title?: React.ReactNode
  side?: 'top' | 'right' | 'bottom' | 'left'
  delay?: number
  className?: string
  children: React.ReactElement
}

export function Tooltip({ content, title, side = 'top', delay = 300, className, children }: TooltipProps) {
  const body = content ?? title
  if (body == null) return children
  return (
    <TooltipProvider delayDuration={delay}>
      <TT>
        <TooltipTrigger asChild>{children}</TooltipTrigger>
        <TooltipContent side={side} className={className}>{body}</TooltipContent>
      </TT>
    </TooltipProvider>
  )
}
