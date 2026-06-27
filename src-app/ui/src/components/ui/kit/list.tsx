import * as React from 'react'
import { cn } from '@/lib/utils'
import { Skeleton } from '../shadcn/skeleton'
import { useSurface } from './surface'
import { Empty } from './empty'

// legacy List (simple form): dataSource + renderItem. Renders a semantic <ul>.
export interface ListProps<T> {
  dataSource: T[]
  renderItem: (item: T, index: number) => React.ReactNode
  rowKey: (item: T, index: number) => string
  header?: React.ReactNode
  footer?: React.ReactNode
  empty?: React.ReactNode
  /** Own busy state (legacy `loading`) → skeleton rows. Region loading (surface) also applies. */
  loading?: boolean
  size?: 'sm' | 'default' | 'lg'
  className?: string
  'aria-label'?: string
}

const rowPad = (size?: 'sm' | 'default' | 'lg') => (size === 'sm' ? 'px-3 py-2' : size === 'lg' ? 'px-5 py-4' : 'px-4 py-3')

export function List<T>({ dataSource, renderItem, rowKey, header, footer, empty, loading, size, className, 'aria-label': ariaLabel }: ListProps<T>) {
  const s = useSurface({})
  const busy = loading || s.loading
  return (
    <div className={cn('rounded-md border', className)}>
      {header != null && <div className={cn('border-b font-medium', rowPad(size))}>{header}</div>}
      {busy ? (
        <ul className="divide-y">
          {Array.from({ length: 3 }).map((_, i) => (
            <li key={`sk-${i}`} className={rowPad(size)}><Skeleton className="h-4 w-full" /></li>
          ))}
        </ul>
      ) : dataSource.length === 0 ? (
        <div className="p-6">{empty ?? <Empty />}</div>
      ) : (
        <ul aria-label={ariaLabel} className="divide-y">
          {dataSource.map((item, i) => (
            <li key={rowKey(item, i)} className={rowPad(size)}>{renderItem(item, i)}</li>
          ))}
        </ul>
      )}
      {footer != null && <div className={cn('border-t', rowPad(size))}>{footer}</div>}
    </div>
  )
}
