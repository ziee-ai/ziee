import * as React from 'react'
import { type KitStyleProps } from './style-guard'
import { cn } from '@/lib/utils'

export interface DescriptionsItem {
  key: string
  label: React.ReactNode
  children: React.ReactNode
  span?: number
}

// legacy Descriptions: a label/value detail grid. Rendered as a semantic <dl>.
export type DescriptionsProps = {
  items: DescriptionsItem[]
  title?: React.ReactNode
  /** Number of value columns (default 1). */
  column?: number
  bordered?: boolean
  size?: 'sm' | 'default'
  /** Test selector — forwarded onto <root> (i18n-safe). */
  'data-testid': string
  className?: string} & KitStyleProps

export function Descriptions({ items, title, column = 1, bordered, size = 'default', className, style, 'data-testid': testid }: DescriptionsProps) {
  const pad = size === 'sm' ? 'px-3 py-1.5 text-xs' : 'px-4 py-2 text-sm'
  return (
    <div className={cn('w-full', className)} style={style} data-testid={testid}>
      {title != null && <div className="mb-2 font-semibold">{title}</div>}
      <dl
        className={cn('grid', bordered && 'overflow-hidden rounded-md border')}
        style={{ gridTemplateColumns: `repeat(${column}, minmax(0, auto) minmax(0, 1fr))` }}
      >
        {items.map((it) => (
          <React.Fragment key={it.key}>
            <dt className={cn(pad, 'font-medium', bordered && 'border-b')}>{it.label}</dt>
            <dd
              className={cn(pad, 'min-w-0 [overflow-wrap:anywhere]', bordered && 'border-b')}
              style={it.span && it.span > 1 ? { gridColumn: `span ${it.span * 2 - 1}` } : undefined}
            >
              {it.children}
            </dd>
          </React.Fragment>
        ))}
      </dl>
    </div>
  )
}
