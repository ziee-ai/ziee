import * as React from 'react'
import { Separator as Base } from '../shadcn/separator'
import { cn } from '@/lib/utils'

export interface SeparatorProps {
  orientation?: 'horizontal' | 'vertical'
  children?: React.ReactNode
  /** Position of the label text on a labeled divider (legacy `titlePlacement`). Default 'center'. */
  titlePlacement?: 'left' | 'center' | 'right'
  className?: string
}

export function Separator({ orientation = 'horizontal', children, titlePlacement = 'center', className }: SeparatorProps) {
  if (children) {
    // labeled divider: the inner <Base> elements carry the separator role; the wrapper
    // must NOT also be role="separator" (a separator can't contain meaningful text).
    // titlePlacement shrinks the leading/trailing line so the label sits left/right/center.
    const lead = titlePlacement === 'left' ? 'w-4 flex-none' : 'flex-1'
    const trail = titlePlacement === 'right' ? 'w-4 flex-none' : 'flex-1'
    return (
      <div className={cn('flex items-center gap-3 text-xs text-muted-foreground', className)}>
        <Base className={lead} />
        <span className="shrink-0">{children}</span>
        <Base className={trail} />
      </div>
    )
  }
  return <Base orientation={orientation} className={className} />
}
