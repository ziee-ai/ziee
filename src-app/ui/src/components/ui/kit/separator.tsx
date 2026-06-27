import * as React from 'react'
import { Separator as Base } from '../shadcn/separator'
import { cn } from '@/lib/utils'

export interface SeparatorProps {
  orientation?: 'horizontal' | 'vertical'
  children?: React.ReactNode
  className?: string
}

export function Separator({ orientation = 'horizontal', children, className }: SeparatorProps) {
  if (children) {
    // labeled divider: the inner <Base> elements carry the separator role; the wrapper
    // must NOT also be role="separator" (a separator can't contain meaningful text).
    return (
      <div className={cn('flex items-center gap-3 text-xs text-muted-foreground', className)}>
        <Base className="flex-1" />
        <span className="shrink-0">{children}</span>
        <Base className="flex-1" />
      </div>
    )
  }
  return <Base orientation={orientation} className={className} />
}
