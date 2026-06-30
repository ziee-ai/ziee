import * as React from 'react'
import { Card as Base, CardHeader, CardTitle, CardContent, CardFooter } from '../shadcn/card'
import { Skeleton } from '../shadcn/skeleton'
import { useSurface } from './surface'
import { type KitStyleProps } from './style-guard'
import { cn } from '@/lib/utils'

export type CardProps = Omit<React.ComponentProps<'div'>, 'title' | 'style'> & {
  title?: React.ReactNode
  extra?: React.ReactNode
  footer?: React.ReactNode
  loading?: boolean
  size?: 'sm' | 'default'
  hoverable?: boolean
  className?: string
  children?: React.ReactNode
  'data-testid': string
} & KitStyleProps

export function Card({ title, extra, footer, loading, size = 'default', hoverable, className, style, allowStyle: _a, children, ...rest }: CardProps) {
  const s = useSurface({})
  const skeleton = loading || s.loading
  const pad = size === 'sm' ? undefined : undefined
  return (
    <Base
      style={style}
      className={cn(size === 'sm' && 'gap-3', hoverable && 'transition-shadow hover:shadow-md', rest.onClick && 'cursor-pointer', className)}
      {...rest}
    >
      {(title != null || extra != null) && (
        <CardHeader className={cn('flex flex-row items-center justify-between gap-2', pad)}>
          {title != null ? (
            <CardTitle className="min-w-0 [overflow-wrap:anywhere]">{title}</CardTitle>
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
