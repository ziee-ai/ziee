import * as React from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import type { OverlayScrollbarsComponentRef } from 'overlayscrollbars-react'
import {
  Table as Base, TableHeader, TableBody, TableRow, TableHead, TableCell, TableCaption,
} from '../shadcn/table'
import { Skeleton } from '../shadcn/skeleton'
import { useSurface } from './surface'
import { ScrollArea } from './scroll-area'
import { Empty } from './empty'
import { cn } from '@/lib/utils'

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
  /** Row-virtualize the body (only visible rows mount) for large data sets.
   *  Requires a bounded-height scroll ancestor (the kit ScrollArea / any
   *  overflow box); column `width`s are used for the flex row layout. */
  virtualized?: boolean
  /** Estimated row height (px) for the virtualizer; real heights are measured. */
  estimateRowHeight?: number
  /** Max height (CSS length) of the virtualized scroll box; short tables shrink
   *  to fit, taller ones cap here and scroll. Default `min(60vh, 36rem)`. */
  maxHeight?: string
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

export function Table<T>(props: TableProps<T>) {
  const s = useSurface({})
  const busy = !!(props.loading || s.loading)
  // Virtualize only when there's real data to window — loading/empty states use
  // the plain path (their skeleton/empty rows don't need a virtualizer).
  if (props.virtualized && !busy && props.dataSource.length > 0) {
    return <VirtualTable {...props} />
  }
  return <PlainTable {...props} busy={busy} />
}

function PlainTable<T>({ columns, dataSource, rowKey, caption, empty, className, onRowClick, busy, 'data-testid': testid }: TableProps<T> & { busy: boolean }) {
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

// Row-virtualized table (TanStack `useVirtualizer`), used for large data grids.
// Grid/flex layout + absolutely-positioned rows so only the visible window mounts
// while the sticky header + horizontal scroll still work. It finds the actual
// scroll element (the kit ScrollArea's OverlayScrollbars viewport, or any
// overflow ancestor) so it can live inside our overlay scrollers unchanged.
const justifyFor = { left: 'justify-start', center: 'justify-center', right: 'justify-end' } as const
function VirtualTable<T>({
  columns, dataSource, rowKey, className, onRowClick, estimateRowHeight = 40,
  maxHeight = 'min(60vh, 36rem)', 'data-testid': testid,
}: TableProps<T>) {
  // Own the scroll container so the virtualizer reads a ref that's populated by
  // commit time — react-virtual attaches its scroll observers once at mount, so
  // a ref (not effect-set state) is required. The kit ScrollArea inits its
  // OverlayScrollbars in a child effect (runs before this component's mount
  // effect), so the viewport is available when the virtualizer attaches.
  const osRef = React.useRef<OverlayScrollbarsComponentRef>(null)
  const getScrollElement = React.useCallback(
    () => osRef.current?.osInstance()?.elements().viewport ?? null,
    [],
  )

  const keyOf = (record: T, i: number) =>
    typeof rowKey === 'function' ? rowKey(record, i) : String((record as Record<string, unknown>)[rowKey])

  const virt = useVirtualizer({
    count: dataSource.length,
    getScrollElement,
    estimateSize: () => estimateRowHeight,
    overscan: 12,
  })
  const items = virt.getVirtualItems()
  const colStyle = (c: TableColumn<T>): React.CSSProperties => ({ display: 'flex', width: c.width ?? 160, flexShrink: 0 })

  return (
    <ScrollArea
      ref={osRef}
      axis="both"
      autoHide="leave"
      style={{ maxHeight }}
      className="w-full rounded-md border border-border bg-background"
    >
      <table data-testid={testid} className={cn('w-max text-sm', className)} style={{ display: 'grid' }}>
        <thead className="sticky top-0 z-[1] bg-muted/80" style={{ display: 'grid' }}>
          <tr className="border-b border-border" style={{ display: 'flex', width: '100%' }}>
            {columns.map((c) => (
              <th key={c.key} style={colStyle(c)} className={cn('px-4 py-2 font-semibold', justifyFor[c.align ?? 'left'])}>
                {c.title}
              </th>
            ))}
          </tr>
        </thead>
        <tbody style={{ display: 'grid', height: virt.getTotalSize(), position: 'relative' }}>
          {items.map((vi) => {
            const record = dataSource[vi.index]
            return (
              <tr
                key={keyOf(record, vi.index)}
                data-index={vi.index}
                ref={virt.measureElement}
                data-testid={testid ? `${testid}-row-${keyOf(record, vi.index)}` : undefined}
                onClick={onRowClick ? () => onRowClick(record, vi.index) : undefined}
                className={cn('border-b border-border', onRowClick && 'cursor-pointer hover:bg-muted/50')}
                style={{ display: 'flex', position: 'absolute', top: 0, left: 0, width: '100%', transform: `translateY(${vi.start}px)` }}
              >
                {columns.map((c) => (
                  <td key={c.key} style={{ ...colStyle(c), minWidth: 0 }} className={cn('px-4 py-2', justifyFor[c.align ?? 'left'])}>
                    {c.render ? c.render(record, vi.index) : defaultCell((record as Record<string, unknown>)[c.dataIndex ?? c.key])}
                  </td>
                ))}
              </tr>
            )
          })}
        </tbody>
      </table>
    </ScrollArea>
  )
}
