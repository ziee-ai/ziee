import * as React from 'react'
import {
  Table as Base, TableHeader, TableBody, TableRow, TableHead, TableCell, TableCaption,
} from '../shadcn/table'
import { Skeleton } from '../shadcn/skeleton'
import { useSurface } from './surface'
import { Empty } from './empty'

// legacy Table (common subset): columns + dataSource. Column.render gets (record, index).
export interface TableColumn<T> {
  key: string
  title: React.ReactNode
  /** Record field to read when `render` is omitted. Defaults to `key`. */
  dataIndex?: string
  render?: (record: T, index: number) => React.ReactNode
  align?: 'left' | 'center' | 'right'
  width?: number | string
}

export interface TableProps<T> {
  columns: TableColumn<T>[]
  dataSource: T[]
  /** Row key: a record field name (legacy string form) or a function. */
  rowKey: (keyof T & string) | ((record: T, index: number) => string)
  /** Own loading → in-place skeleton rows. Region loading (surface) → skeleton too. */
  loading?: boolean
  caption?: React.ReactNode
  empty?: React.ReactNode
  className?: string
  onRowClick?: (record: T, index: number) => void
  /** Test selector — forwarded onto <root>. Rows derive `${testid}-row-${rowKey}`. */
  'data-testid': string
}

const alignCls = { left: 'text-left', center: 'text-center', right: 'text-right' } as const

// Default cell: render primitives directly; stringify anything else (a raw object/Date as a
// React child would throw). Columns needing rich content must supply `render`.
function defaultCell(v: unknown): React.ReactNode {
  if (v == null || typeof v === 'boolean') return null
  if (typeof v === 'object' && !React.isValidElement(v)) return String(v)
  return v as React.ReactNode
}

export function Table<T>({ columns, dataSource, rowKey, loading, caption, empty, className, onRowClick, 'data-testid': testid }: TableProps<T>) {
  const s = useSurface({})
  const busy = loading || s.loading
  const keyOf = (record: T, i: number) =>
    typeof rowKey === 'function' ? rowKey(record, i) : String((record as Record<string, unknown>)[rowKey])
  return (
    <Base className={className} data-testid={testid}>
      {caption != null && <TableCaption>{caption}</TableCaption>}
      <TableHeader>
        <TableRow>
          {columns.map((c) => (
            <TableHead key={c.key} style={{ width: c.width }} className={c.align ? alignCls[c.align] : undefined}>
              {c.title}
            </TableHead>
          ))}
        </TableRow>
      </TableHeader>
      <TableBody>
        {busy ? (
          Array.from({ length: 3 }).map((_, r) => (
            <TableRow key={`sk-${r}`}>
              {columns.map((c) => (
                <TableCell key={c.key}><Skeleton className="h-4 w-full" /></TableCell>
              ))}
            </TableRow>
          ))
        ) : dataSource.length === 0 ? (
          <TableRow>
            <TableCell colSpan={columns.length} className="h-24">{empty ?? <Empty data-testid={`${testid}-empty`} />}</TableCell>
          </TableRow>
        ) : (
          dataSource.map((record, i) => (
            <TableRow
              key={keyOf(record, i)}
              data-testid={testid ? `${testid}-row-${keyOf(record, i)}` : undefined}
              // Keyboard-operable when clickable: focusable + Enter/Space activate. We keep the
              // native row semantics (no role override — role="button" on a <tr> is invalid ARIA
              // and breaks cell association).
              tabIndex={onRowClick ? 0 : undefined}
              onClick={onRowClick ? () => onRowClick(record, i) : undefined}
              onKeyDown={
                onRowClick
                  ? (e) => {
                      if (e.key === 'Enter' || e.key === ' ') {
                        e.preventDefault()
                        onRowClick(record, i)
                      }
                    }
                  : undefined
              }
              className={onRowClick ? 'cursor-pointer focus-visible:outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50' : undefined}
            >
              {columns.map((c) => (
                <TableCell key={c.key} className={c.align ? alignCls[c.align] : undefined}>
                  {c.render ? c.render(record, i) : defaultCell((record as Record<string, unknown>)[c.dataIndex ?? c.key])}
                </TableCell>
              ))}
            </TableRow>
          ))
        )}
      </TableBody>
    </Base>
  )
}
