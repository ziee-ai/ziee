import * as React from 'react'
import { Progress as Base } from '../shadcn/progress'
import { cn } from '@/lib/utils'

export type ProgressTone = 'primary' | 'success' | 'warning' | 'error'
const toneCls: Record<ProgressTone, string> = {
  primary: '[&>[data-state]]:bg-primary [&>div]:bg-primary',
  success: '[&>[data-state]]:bg-green-600 [&>div]:bg-green-600',
  warning: '[&>[data-state]]:bg-amber-500 [&>div]:bg-amber-500',
  error: '[&>[data-state]]:bg-destructive [&>div]:bg-destructive',
}

export interface ProgressProps {
  value: number
  tone?: ProgressTone
  size?: 'sm' | 'default'
  /** Show the percentage text beside the bar (legacy `showInfo`). */
  showInfo?: boolean
  /** Custom label formatter (legacy `format`), e.g. (p) => `${p}%`. Implies showInfo. */
  format?: (percent: number) => React.ReactNode
  className?: string
  /** Required accessible name (no default — caller owns the string for i18n). */
  'aria-label': string
}

export function Progress({ value, tone = 'primary', size = 'default', showInfo, format, className, 'aria-label': ariaLabel }: ProgressProps) {
  const v = Math.max(0, Math.min(100, Number.isFinite(value) ? value : 0))
  const bar = (
    <Base
      value={v}
      aria-label={ariaLabel}
      aria-valuetext={`${Math.round(v)}%`}
      className={cn(toneCls[tone], size === 'sm' && 'h-1.5', !showInfo && !format && className)}
    />
  )
  if (!showInfo && !format) return bar
  return (
    <div className={cn('flex items-center gap-2', className)}>
      <div className="flex-1">{bar}</div>
      <span className="shrink-0 text-sm tabular-nums text-muted-foreground">
        {format ? format(Math.round(v)) : `${Math.round(v)}%`}
      </span>
    </div>
  )
}
