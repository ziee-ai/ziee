import * as React from 'react'
import { type KitStyleProps } from './style-guard'
import { cn } from '@/lib/utils'

export interface DescriptionsItem {
  key: string
  label: React.ReactNode
  children: React.ReactNode
  span?: number
}

// legacy Descriptions: a label/value detail grid. Rendered as a semantic <table>
// (the way Ant Design does it) rather than a CSS grid — a table with automatic
// layout is inherently responsive: on a narrow container the cells shrink and
// their content wraps instead of overflowing (a grid with `auto` label tracks
// can't shrink and pushes the row wider than the screen).
export type DescriptionsProps = {
  items: DescriptionsItem[]
  title?: React.ReactNode
  /** Number of label/value pairs per row (default 1). */
  column?: number
  bordered?: boolean
  size?: 'sm' | 'default'
  /** Test selector — forwarded onto <root> (i18n-safe). */
  'data-testid': string
  className?: string} & KitStyleProps

export function Descriptions({ items, title, column = 1, bordered, size = 'default', className, style, 'data-testid': testid }: DescriptionsProps) {
  const pad = size === 'sm' ? 'px-3 py-1.5 text-xs' : 'px-4 py-2 text-sm'
  const cols = Math.max(1, column)

  // Chunk items into rows of at most `cols` "units" (an item with span S takes S
  // units), so multi-column layouts reflow into rows exactly like Ant Design.
  const rows: DescriptionsItem[][] = []
  let cur: DescriptionsItem[] = []
  let used = 0
  for (const it of items) {
    const span = Math.min(Math.max(it.span ?? 1, 1), cols)
    if (used + span > cols && cur.length) {
      rows.push(cur)
      cur = []
      used = 0
    }
    cur.push(it)
    used += span
    if (used >= cols) {
      rows.push(cur)
      cur = []
      used = 0
    }
  }
  if (cur.length) rows.push(cur)

  return (
    <div className={cn('w-full', className)} style={style} data-testid={testid}>
      {title != null && <div className="mb-2 font-semibold">{title}</div>}
      <div className={cn(bordered && 'overflow-hidden rounded-md border')}>
        <table className="w-full border-collapse text-left">
          <tbody>
            {rows.map((row, ri) => {
              const isLastRow = ri === rows.length - 1
              return (
                <tr key={ri}>
                  {row.map((it) => {
                    // A span-S value cell absorbs the (S-1) label columns it skips,
                    // matching the grid's `span * 2 - 1` behavior.
                    const valueColSpan = it.span && it.span > 1 ? it.span * 2 - 1 : 1
                    return (
                      <React.Fragment key={it.key}>
                        <th
                          scope="row"
                          className={cn(
                            pad,
                            'w-px whitespace-nowrap align-top font-medium text-muted-foreground',
                            bordered && cn('border-r bg-muted/40', !isLastRow && 'border-b'),
                          )}
                        >
                          {it.label}
                        </th>
                        <td
                          colSpan={valueColSpan}
                          className={cn(
                            pad,
                            'min-w-0 align-top [overflow-wrap:anywhere]',
                            bordered && cn('border-r last:border-r-0', !isLastRow && 'border-b'),
                          )}
                        >
                          {it.children}
                        </td>
                      </React.Fragment>
                    )
                  })}
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>
    </div>
  )
}
