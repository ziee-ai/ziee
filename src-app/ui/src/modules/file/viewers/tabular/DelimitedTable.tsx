import { useEffect, useMemo, useRef, useState } from 'react'
import { Table, Alert } from 'antd'
import type { TableColumnsType } from 'antd'

/** Cap on rendered rows. Above this, the table is truncated to the
 *  first N and a banner offers Download for full content. The wider 8
 *  MB byte-cap at FilePanel still applies upstream — by the time we
 *  get here the file is already under that bound. `virtual` on the
 *  antd Table keeps row rendering cheap at this size. */
const MAX_ROWS = 10_000

/** Rough height in pixels of the antd Table header row + the small
 *  internal padding the virtual table reserves. Used to subtract from
 *  the measured container height so the scrollable body fills the
 *  remaining space exactly. Off-by-a-few-pixels is harmless — the
 *  body just scrolls slightly more or less than perfect. */
const TABLE_HEADER_PX = 48

function parseDelimitedLine(line: string, delimiter: string): string[] {
  const fields: string[] = []
  let field = ''
  let inQuotes = false

  for (let i = 0; i < line.length; i++) {
    const ch = line[i]
    if (ch === '"') {
      if (inQuotes && line[i + 1] === '"') { field += '"'; i++ }
      else inQuotes = !inQuotes
    } else if (ch === delimiter && !inQuotes) {
      fields.push(field.trim())
      field = ''
    } else {
      field += ch
    }
  }
  fields.push(field.trim())
  return fields
}

function parseDelimitedText(text: string, delimiter: string): { headers: string[]; rows: string[][]; truncated: boolean } {
  const lines = text.split('\n').filter(l => l.trim() !== '')
  if (lines.length === 0) return { headers: [], rows: [], truncated: false }
  const headers = parseDelimitedLine(lines[0], delimiter)
  const dataLines = lines.slice(1)
  const truncated = dataLines.length > MAX_ROWS
  const rows = dataLines.slice(0, MAX_ROWS).map(l => parseDelimitedLine(l, delimiter))
  return { headers, rows, truncated }
}

export function DelimitedTable({ text, delimiter }: { text: string; delimiter: string }) {
  // Parse + column/dataSource construction is the entire cost of this
  // component. Memoize on (text, delimiter) so panel re-renders for
  // unrelated reasons (resize, drawer, sibling state) don't re-parse the
  // whole file. AntD's <Table> handles its own internal memoization.
  const { columns, dataSource, truncated, scrollX } = useMemo(() => {
    const { headers, rows, truncated } = parseDelimitedText(text, delimiter)
    const ROW_NUM_WIDTH = 56
    const COL_WIDTH = 240
    // Row-number gutter column. Width fits a 5-digit count
    // (10,000 cap); fixed-left so it stays anchored when scrolling
    // wide tables horizontally.
    const rowNumberColumn: TableColumnsType<Record<string, string>>[number] = {
      title: '#',
      dataIndex: '__rn',
      key: '__rn',
      width: ROW_NUM_WIDTH,
      fixed: 'left',
      align: 'right',
      render: (v: string) => (
        <span style={{ opacity: 0.5, fontVariantNumeric: 'tabular-nums' }}>
          {v}
        </span>
      ),
    }
    // Pre-compute column keys once — building 7k rows × N cols would
    // otherwise call `String(i)` 7k×N times in the dataSource loop.
    const colKeys = headers.map((_, i) => String(i))
    const dataColumns: TableColumnsType<Record<string, string>> = headers.map((h, i) => ({
      title: h || `Column ${i + 1}`,
      dataIndex: colKeys[i],
      key: colKeys[i],
      width: COL_WIDTH,
      ellipsis: { showTitle: true },
    }))
    const columns = [rowNumberColumn, ...dataColumns]
    const dataSource = rows.map((row, ri) => {
      const record: Record<string, string> = {
        key: String(ri),
        __rn: String(ri + 1),
      }
      for (let i = 0; i < colKeys.length; i++) {
        record[colKeys[i]] = row[i] ?? ''
      }
      return record
    })
    // Explicit pixel width for scroll.x — antd's virtual mode
    // partially disables when scroll.x is the boolean `true`. With
    // a number it can compute horizontal offsets and only paint
    // visible columns.
    const scrollX = ROW_NUM_WIDTH + headers.length * COL_WIDTH
    return { columns, dataSource, truncated, scrollX }
  }, [text, delimiter])

  // Measure the table's available area via ResizeObserver and pass the
  // computed body height to antd. Without a number for scroll.y,
  // antd silently disables virtual mode and renders ALL rows into the
  // DOM — for a 7k-row CSV that's a ~10 second initial paint. We seed
  // bodyHeight with a sensible non-zero default so the first paint
  // is already virtualized; the ResizeObserver then refines the
  // exact height once the layout settles. The Alert (when truncated)
  // lives outside the measured element — its height doesn't reduce
  // the table area.
  const wrapRef = useRef<HTMLDivElement>(null)
  const [bodyHeight, setBodyHeight] = useState<number>(600)

  useEffect(() => {
    if (!wrapRef.current) return
    const ro = new ResizeObserver(entries => {
      for (const entry of entries) {
        const h = entry.contentRect.height
        const scrollY = Math.max(0, Math.floor(h) - TABLE_HEADER_PX)
        if (scrollY > 0) setBodyHeight(scrollY)
      }
    })
    ro.observe(wrapRef.current)
    return () => ro.disconnect()
  }, [])

  return (
    <div className="flex flex-col h-full w-full px-2">
      {truncated && (
        <Alert
          title={`Showing first ${MAX_ROWS.toLocaleString()} rows. Download the file to view all data.`}
          type="warning"
          showIcon
          className="mb-2 flex-shrink-0"
        />
      )}
      <div ref={wrapRef} className="flex-1 min-h-0 w-full">
        <Table
          columns={columns}
          dataSource={dataSource}
          size="small"
          scroll={{ x: scrollX, y: bodyHeight }}
          pagination={false}
          virtual
          locale={{ emptyText: 'No data to display' }}
        />
      </div>
    </div>
  )
}
