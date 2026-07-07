import * as React from 'react'
import { Card as Base, CardHeader, CardTitle, CardContent, CardFooter } from '../shadcn/card'
import { Skeleton } from '../shadcn/skeleton'
import { useSurface } from './surface'
import { type KitStyleProps } from './style-guard'
import { cn } from '@/lib/utils'

// Omit native `title` (we take a ReactNode title) + `style` (style-gated). The rest of the
// div props (onClick, data-*, role, id, aria-*) pass through to the card root.
export type CardProps = Omit<React.ComponentProps<'div'>, 'title' | 'style'> & {
  title?: React.ReactNode
  /** Top-right actions. */
  extra?: React.ReactNode
  footer?: React.ReactNode
  /** Container is loading → skeleton body (also triggered by an ambient loading surface). */
  loading?: boolean
  size?: 'sm' | 'default'
  /** Lift + shadow on hover (legacy `hoverable`). */
  hoverable?: boolean
  className?: string
  children?: React.ReactNode
  /** Test selector — REQUIRED, forwarded onto the card root via {...rest} (i18n-safe). */
  'data-testid': string
} & KitStyleProps

export function Card({ title, extra, footer, loading, size = 'default', hoverable, className, style, allowStyle: _a, children, ...rest }: CardProps) {
  const s = useSurface({})
  const skeleton = loading || s.loading
  const pad = size === 'sm' ? 'px-4' : undefined
  return (
    <Base
      style={style}
      className={cn(size === 'sm' && 'gap-3 py-4', hoverable && 'transition-shadow hover:shadow-md', rest.onClick && 'cursor-pointer', className)}
      {...rest}
    >
      {/* Header stacks title + extra on mobile so a wide `extra` (e.g. a
          "Check for updates" button) can't starve the title into a mid-word
          wrap; restores the single row from sm up. */}
      {(title != null || extra != null) && (
        <CardHeader className={cn('flex flex-col items-start gap-2 sm:flex-row sm:items-center sm:justify-between', pad)}>
          {title != null ? (
            // `sm:flex-1` (row layout only): let the title claim the header's
            // free width instead of shrinking to its min-content next to a
            // right-aligned `extra` — a shrink-to-fit title wraps "with room"
            // (e.g. a short project name breaking mid-phrase on tablet). Scoped
            // to `sm:` so the mobile `flex-col` header doesn't stretch the title
            // vertically.
            <CardTitle className="min-w-0 sm:flex-1 [overflow-wrap:anywhere]">{title}</CardTitle>
          ) : (
            <span />
          )}
          {extra}
        </CardHeader>
      )}
      <CardContent className={pad}>
        {skeleton ? (
          <div className="space-y-2" aria-busy>
            <Skeleton className="h-4 w-3/4" />
            <Skeleton className="h-4 w-1/2" />
            <Skeleton className="h-4 w-2/3" />
          </div>
        ) : (
          children
        )}
      </CardContent>
      {footer != null && <CardFooter className={pad}>{footer}</CardFooter>}
    </Base>
  )
}
