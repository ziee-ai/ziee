import * as React from 'react'
import { cn } from '@/lib/utils'
import { type KitStyleProps } from './style-guard'

export type BadgeTone = 'neutral' | 'primary' | 'success' | 'warning' | 'error' | 'info'

const tones: Record<BadgeTone, string> = {
  neutral: 'bg-muted text-muted-foreground',
  primary: 'bg-primary/10 text-primary',
  success: 'bg-green-500/10 text-green-700 dark:text-green-400',
  warning: 'bg-amber-500/10 text-amber-700 dark:text-amber-500',
  error: 'bg-destructive/10 text-destructive',
  info: 'bg-blue-500/10 text-blue-700 dark:text-blue-400',
}

export type BadgeProps = Omit<React.ComponentProps<'span'>, 'prefix' | 'style'> & {
  tone?: BadgeTone
  icon?: React.ReactNode
} & KitStyleProps

export const Badge = React.forwardRef<HTMLSpanElement, BadgeProps>(
  ({ tone = 'neutral', icon, style, allowStyle, className, children, ...props }, ref) => (
    <span
      ref={ref}
      style={style}
      className={cn(
        'inline-flex items-center gap-1 whitespace-nowrap rounded-md px-2 py-0.5 text-xs font-medium [&_svg]:size-3',
        tones[tone],
        className,
      )}
      {...props}
    >
      {icon != null && <span aria-hidden>{icon}</span>}
      {children}
    </span>
  ),
)
Badge.displayName = 'Badge'
